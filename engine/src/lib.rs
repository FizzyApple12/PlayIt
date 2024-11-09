use std::{io::Read, time::Duration};

use ipc::{client::IPCClient, server::IPCServer};
use player::{database::Database, sequencer::Sequencer, PlaylistMetadata, RecordingMetadata};
use tokio::{
    sync::{
        broadcast,
        mpsc::{self},
    },
    task::JoinHandle,
};

mod ipc;
mod player;

pub struct Engine {
    sequencer: Sequencer,
    database: Database,

    location: EngineLocation,

    engine_command_sender: broadcast::Sender<EngineCommand>,
    engine_response_sender: broadcast::Sender<EngineResponse>,
}
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde()]
pub enum LoopMode {
    None,
    LoopQueue,
    LoopRecording,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde()]
pub enum Permission {
    Control,
    Queue,
    Playlist,
    Transfer,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type")]
pub enum EngineCommand {
    None,
    Goodbye,

    Play(Option<String>),
    Pause,

    Next,
    Previous,

    Seek(Duration),

    Queue(Option<Vec<String>>),
    ShuffleQueue(bool),
    ClearQueue,

    LoopMode(LoopMode),

    RecordingMetadata(String),
    RecordingFile(String),
    SendRecording((String, Vec<u8>)),

    PlaylistMetadata(String),
    SetPlaylistMetadata(PlaylistMetadata),

    SetVolume(f32),

    GetPermissions,
    SetPermissions(Vec<Permission>),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type")]
pub enum EngineResponse {
    Ok(EngineCommand),
    Nope(EngineCommand),

    NowPlaying(String),
    NowPaused,

    Seek(Duration),
    CurrentTime(Duration),

    Queue(Vec<String>),

    LoopMode(LoopMode),

    RecordingMetadata(RecordingMetadata),
    RecordingFile((String, Vec<u8>)),

    PlaylistMetadata(PlaylistMetadata),

    Permissions(Vec<Permission>),
}

pub enum EngineLocation {
    Invalid,
    Internal {
        ipc_server: IPCServer,
        command_processor: JoinHandle<()>,
    },
    Local {
        ipc_client: IPCClient,
        command_relay: JoinHandle<()>,
    },
    Remote {
        ipc_client: IPCClient,
        command_relay: JoinHandle<()>,
    },
}

pub enum EngineError {
    AudioInitializationFailed,
    DatabaseInitializationFailed,
}

pub enum EngineConnectionStatus {
    ConnectedLocal,
    ConnectedRemote,

    Disconnected,
}

pub enum EngineLocalConnectionError {
    StartFailed,
}

pub enum EngineRemoteConnectionError {
    InvalidAddress,
    ConnectionFailed,
}

pub enum EngineCommandError {
    Disconnected,
}

impl Engine {
    pub fn create() -> Result<
        (
            Engine,
            broadcast::Sender<EngineCommand>,
            broadcast::Receiver<EngineResponse>,
        ),
        EngineError,
    > {
        let (engine_command_sender, _) = broadcast::channel::<EngineCommand>(16);
        let (engine_response_sender, engine_response_receiver) =
            broadcast::channel::<EngineResponse>(16);

        let Ok(database) = Database::new() else {
            return Err(EngineError::DatabaseInitializationFailed);
        };
        let Ok(sequencer) = Sequencer::new(database.clone()) else {
            return Err(EngineError::AudioInitializationFailed);
        };

        let mut new_engine = Engine {
            sequencer,
            database,
            location: EngineLocation::Invalid,
            engine_command_sender: engine_command_sender.clone(),
            engine_response_sender,
        };

        let _ = new_engine.connect_to_local();

        Ok((new_engine, engine_command_sender, engine_response_receiver))
    }

    fn start_command_processor(
        &mut self,
        mut command_receiver: mpsc::Receiver<(EngineCommand, Uuid)>,
        response_sender: broadcast::Sender<(EngineResponse, Uuid)>,
    ) -> JoinHandle<()> {
        let mut internal_command_receiver = self.engine_command_sender.subscribe();
        let internal_response_sender = self.engine_response_sender.clone();

        let database = self.database.clone();
        let sequencer = self.sequencer.clone();

        tokio::spawn(async move {
            let mut current_user_permissions = Vec::<Permission>::new();

            loop {
                let (command, uuid, internal) = tokio::select! {
                    val = internal_command_receiver.recv() => {
                        let Ok(command) = val else {
                            continue;
                        };

                        (command, Uuid::nil(), true)
                    }
                    val = command_receiver.recv() => {
                        let Some((command, uuid)) = val else {
                            continue;
                        };

                        (command, uuid, false)
                    }
                };

                match command {
                    EngineCommand::None | EngineCommand::Goodbye => {
                        route_response(
                            internal,
                            &internal_response_sender,
                            &response_sender,
                            EngineResponse::Ok(command),
                            uuid,
                        );
                    }
                    EngineCommand::Play(id) => {
                        let Some(id) = id else {
                            route_response(
                                internal,
                                &internal_response_sender,
                                &response_sender,
                                if let Some(id) = sequencer.get_playing().await {
                                    EngineResponse::NowPlaying(id)
                                } else {
                                    EngineResponse::NowPaused
                                },
                                Uuid::nil(),
                            );

                            continue;
                        };

                        if !internal
                            && !permission_exists(&current_user_permissions, Permission::Control)
                        {
                            let _ = response_sender
                                .send((EngineResponse::Nope(EngineCommand::Play(Some(id))), uuid));

                            continue;
                        }

                        if sequencer.play(id.clone()).await.is_ok() {
                            route_response(
                                internal,
                                &internal_response_sender,
                                &response_sender,
                                if let Some(id) = sequencer.get_playing().await {
                                    EngineResponse::NowPlaying(id)
                                } else {
                                    EngineResponse::NowPaused
                                },
                                Uuid::nil(),
                            );
                            route_response(
                                internal,
                                &internal_response_sender,
                                &response_sender,
                                EngineResponse::Queue(sequencer.get_queue().await),
                                Uuid::nil(),
                            );
                        } else {
                            route_response(
                                internal,
                                &internal_response_sender,
                                &response_sender,
                                EngineResponse::Nope(EngineCommand::Play(Some(id))),
                                uuid,
                            );
                        }
                    }
                    EngineCommand::Pause => {
                        if !internal
                            && !permission_exists(&current_user_permissions, Permission::Control)
                        {
                            let _ = response_sender.send((EngineResponse::Nope(command), uuid));

                            continue;
                        }

                        sequencer.pause().await;

                        route_response(
                            internal,
                            &internal_response_sender,
                            &response_sender,
                            if let Some(id) = sequencer.get_playing().await {
                                EngineResponse::NowPlaying(id)
                            } else {
                                EngineResponse::NowPaused
                            },
                            Uuid::nil(),
                        );
                    }
                    EngineCommand::Next => {
                        if !internal
                            && !permission_exists(&current_user_permissions, Permission::Control)
                        {
                            let _ = response_sender.send((EngineResponse::Nope(command), uuid));

                            continue;
                        }

                        if sequencer.next().await.is_ok() {
                            route_response(
                                internal,
                                &internal_response_sender,
                                &response_sender,
                                if let Some(id) = sequencer.get_playing().await {
                                    EngineResponse::NowPlaying(id)
                                } else {
                                    EngineResponse::NowPaused
                                },
                                Uuid::nil(),
                            );
                            route_response(
                                internal,
                                &internal_response_sender,
                                &response_sender,
                                EngineResponse::Queue(sequencer.get_queue().await),
                                Uuid::nil(),
                            );
                        } else {
                            route_response(
                                internal,
                                &internal_response_sender,
                                &response_sender,
                                EngineResponse::Nope(EngineCommand::Next),
                                uuid,
                            );
                        }
                    }
                    EngineCommand::Previous => {
                        if !internal
                            && !permission_exists(&current_user_permissions, Permission::Control)
                        {
                            let _ = response_sender.send((EngineResponse::Nope(command), uuid));

                            continue;
                        }

                        if sequencer.previous().await.is_ok() {
                            route_response(
                                internal,
                                &internal_response_sender,
                                &response_sender,
                                if let Some(id) = sequencer.get_playing().await {
                                    EngineResponse::NowPlaying(id)
                                } else {
                                    EngineResponse::NowPaused
                                },
                                Uuid::nil(),
                            );
                            route_response(
                                internal,
                                &internal_response_sender,
                                &response_sender,
                                EngineResponse::Queue(sequencer.get_queue().await),
                                Uuid::nil(),
                            );
                        } else {
                            route_response(
                                internal,
                                &internal_response_sender,
                                &response_sender,
                                EngineResponse::Nope(EngineCommand::Previous),
                                uuid,
                            );
                        }
                    }
                    EngineCommand::Seek(position) => {
                        if !internal
                            && !permission_exists(&current_user_permissions, Permission::Control)
                        {
                            let _ = response_sender.send((EngineResponse::Nope(command), uuid));

                            continue;
                        }

                        if sequencer.seek(position).await.is_ok() {
                            route_response(
                                internal,
                                &internal_response_sender,
                                &response_sender,
                                EngineResponse::Seek(Duration::from_secs(0)),
                                Uuid::nil(),
                            );
                        } else {
                            route_response(
                                internal,
                                &internal_response_sender,
                                &response_sender,
                                EngineResponse::Nope(EngineCommand::Seek(position)),
                                uuid,
                            );
                        }
                    }
                    EngineCommand::Queue(recording_ids) => {
                        let Some(recording_ids) = recording_ids else {
                            route_response(
                                internal,
                                &internal_response_sender,
                                &response_sender,
                                EngineResponse::Queue(sequencer.get_queue().await),
                                Uuid::nil(),
                            );

                            continue;
                        };

                        if !internal
                            && !permission_exists(&current_user_permissions, Permission::Queue)
                        {
                            let _ = response_sender.send((
                                EngineResponse::Nope(EngineCommand::Queue(Some(recording_ids))),
                                uuid,
                            ));

                            continue;
                        }

                        let Ok(not_queued) = sequencer.add_queue(recording_ids.clone()).await
                        else {
                            route_response(
                                internal,
                                &internal_response_sender,
                                &response_sender,
                                EngineResponse::Nope(EngineCommand::Queue(Some(recording_ids))),
                                Uuid::nil(),
                            );

                            continue;
                        };

                        if not_queued.len() != 0 {
                            route_response(
                                internal,
                                &internal_response_sender,
                                &response_sender,
                                EngineResponse::Nope(EngineCommand::Queue(Some(not_queued))),
                                uuid,
                            );
                        }
                        route_response(
                            internal,
                            &internal_response_sender,
                            &response_sender,
                            EngineResponse::Queue(sequencer.get_queue().await),
                            Uuid::nil(),
                        );
                    }
                    EngineCommand::ShuffleQueue(enable) => {
                        if !internal
                            && !permission_exists(&current_user_permissions, Permission::Control)
                        {
                            let _ = response_sender.send((EngineResponse::Nope(command), uuid));

                            continue;
                        }

                        sequencer.set_shuffle(enable).await;

                        route_response(
                            internal,
                            &internal_response_sender,
                            &response_sender,
                            EngineResponse::Queue(sequencer.get_queue().await),
                            Uuid::nil(),
                        );
                    }
                    EngineCommand::ClearQueue => {
                        if !internal
                            && !permission_exists(&current_user_permissions, Permission::Queue)
                        {
                            let _ = response_sender.send((EngineResponse::Nope(command), uuid));

                            continue;
                        }

                        sequencer.clear_queue().await;

                        route_response(
                            internal,
                            &internal_response_sender,
                            &response_sender,
                            EngineResponse::Queue(Vec::new()),
                            Uuid::nil(),
                        );
                    }
                    EngineCommand::LoopMode(loop_mode) => {
                        if !internal
                            && !permission_exists(&current_user_permissions, Permission::Control)
                        {
                            let _ = response_sender.send((
                                EngineResponse::Nope(EngineCommand::LoopMode(loop_mode)),
                                uuid,
                            ));

                            continue;
                        }

                        sequencer.set_loop_mode(loop_mode.clone()).await;

                        route_response(
                            internal,
                            &internal_response_sender,
                            &response_sender,
                            EngineResponse::LoopMode(loop_mode),
                            Uuid::nil(),
                        );
                    }
                    EngineCommand::RecordingMetadata(id) => {
                        let Ok(recording_metadata) =
                            database.get_recording_metadata(id.clone()).await
                        else {
                            route_response(
                                internal,
                                &internal_response_sender,
                                &response_sender,
                                EngineResponse::Nope(EngineCommand::RecordingMetadata(id)),
                                uuid,
                            );
                            continue;
                        };

                        route_response(
                            internal,
                            &internal_response_sender,
                            &response_sender,
                            EngineResponse::RecordingMetadata(recording_metadata),
                            uuid,
                        );
                    }
                    EngineCommand::RecordingFile(id) => {
                        let Ok(mut recording_file) = database.get_recording_file(id.clone()).await
                        else {
                            route_response(
                                internal,
                                &internal_response_sender,
                                &response_sender,
                                EngineResponse::Nope(EngineCommand::RecordingFile(id)),
                                uuid,
                            );
                            continue;
                        };

                        let mut buffer = Vec::new();
                        let _ = recording_file.read_to_end(&mut buffer);

                        route_response(
                            internal,
                            &internal_response_sender,
                            &response_sender,
                            EngineResponse::RecordingFile((id, buffer)),
                            uuid,
                        );
                    }
                    EngineCommand::SendRecording((id, recording)) => {
                        if !internal
                            && !permission_exists(&current_user_permissions, Permission::Transfer)
                        {
                            let _ = response_sender.send((
                                EngineResponse::Nope(EngineCommand::SendRecording((id, recording))),
                                uuid,
                            ));

                            continue;
                        }

                        database
                            .set_recording_file(id.clone(), Some(recording.clone()))
                            .await;

                        route_response(
                            internal,
                            &internal_response_sender,
                            &response_sender,
                            EngineResponse::Ok(EngineCommand::SendRecording((id, recording))),
                            uuid,
                        );
                    }
                    EngineCommand::PlaylistMetadata(id) => {
                        let Ok(playlist_metadata) = database.get_playlist(id.clone()).await else {
                            route_response(
                                internal,
                                &internal_response_sender,
                                &response_sender,
                                EngineResponse::Nope(EngineCommand::PlaylistMetadata(id)),
                                uuid,
                            );
                            continue;
                        };

                        route_response(
                            internal,
                            &internal_response_sender,
                            &response_sender,
                            EngineResponse::PlaylistMetadata(playlist_metadata),
                            uuid,
                        );
                    }
                    EngineCommand::SetPlaylistMetadata(metadata) => {
                        if !internal
                            && !permission_exists(&current_user_permissions, Permission::Playlist)
                        {
                            let _ = response_sender.send((
                                EngineResponse::Nope(EngineCommand::SetPlaylistMetadata(metadata)),
                                uuid,
                            ));

                            continue;
                        }

                        database.set_playlist(metadata.clone()).await;

                        route_response(
                            internal,
                            &internal_response_sender,
                            &response_sender,
                            EngineResponse::PlaylistMetadata(metadata),
                            Uuid::nil(),
                        );
                    }
                    EngineCommand::SetVolume(volume) => {
                        if internal {
                            let _ = sequencer.set_volume(volume).await;
                        }
                    }
                    EngineCommand::GetPermissions => {
                        if internal {
                            let _ =
                                internal_response_sender.send(EngineResponse::Permissions(vec![
                                    Permission::Control,
                                    Permission::Queue,
                                    Permission::Playlist,
                                    Permission::Transfer,
                                ]));
                        } else {
                            let _ = response_sender
                                .send((EngineResponse::Permissions(Vec::new()), uuid));
                        }
                    }
                    EngineCommand::SetPermissions(ref new_permissions) => {
                        if internal {
                            current_user_permissions = new_permissions.to_vec();

                            let _ = internal_response_sender.send(EngineResponse::Permissions(
                                current_user_permissions.clone(),
                            ));
                        } else {
                            let _ = response_sender.send((EngineResponse::Nope(command), uuid));
                        }
                    }
                };
            }
        })
    }

    fn start_command_relay(
        &mut self,
        mut response_receiver: mpsc::Receiver<EngineResponse>,
        command_sender: mpsc::Sender<EngineCommand>,
    ) -> JoinHandle<()> {
        let mut command_receiver = self.engine_command_sender.subscribe();
        let response_sender = self.engine_response_sender.clone();

        let database = self.database.clone();
        let sequencer = self.sequencer.clone();

        tokio::spawn(async move {
            let mut remote_device_permissions = Vec::<Permission>::new();

            loop {
                tokio::select! {
                    response = response_receiver.recv() => if let Some(response) = response {
                        match response {
                            EngineResponse::RecordingMetadata(recording_metadata) => {
                                if permission_exists(&remote_device_permissions, Permission::Transfer) {
                                    let _ = database.get_recording_metadata(recording_metadata.recording.id.clone());
                                }

                                let _ = response_sender.send(EngineResponse::RecordingMetadata(recording_metadata));
                            },
                            EngineResponse::RecordingFile((id, data)) => {
                                if permission_exists(&remote_device_permissions, Permission::Transfer) {
                                    database.set_recording_file(id.clone(), Some(data.clone())).await;
                                }

                                let _ = response_sender.send(EngineResponse::RecordingFile((id, data)));
                            },
                            EngineResponse::PlaylistMetadata(playlist_metadata) => {
                                if permission_exists(&remote_device_permissions, Permission::Playlist) {
                                    database.set_playlist(playlist_metadata.clone()).await;
                                }

                                let _ = response_sender.send(EngineResponse::PlaylistMetadata(playlist_metadata));
                            },
                            x => {
                                let _ = response_sender.send(x);
                            }
                        }
                    },
                    command = command_receiver.recv() => if let Ok(command) = command {
                        match command {
                            EngineCommand::SendRecording((id, data)) => {
                                database.set_recording_file(id.clone(), Some(data.clone())).await;

                                let _ = command_sender.send(EngineCommand::SendRecording((id, data)));
                            },
                            EngineCommand::SetPlaylistMetadata(playlist_metadata) => {
                                database.set_playlist(playlist_metadata.clone()).await;

                                let _ = command_sender.send(EngineCommand::SetPlaylistMetadata(playlist_metadata));
                            },
                            EngineCommand::SetVolume(volume) => {
                                let _ = sequencer.set_volume(volume).await;
                            },
                            EngineCommand::SetPermissions(ref new_permissions) => {
                                remote_device_permissions = new_permissions.to_vec();
                            }
                            x => {
                                let _ = command_sender.send(x);
                            }
                        }
                    }
                }
            }
        })
    }

    pub fn connect_to_local(&mut self) -> Result<(), EngineLocalConnectionError> {
        if matches!(
            self.connection_status(),
            EngineConnectionStatus::ConnectedLocal
        ) {
            return Ok(());
        }

        let Ok((ipc_server, receiver, sender)) = IPCServer::create() else {
            let Ok((ipc_client, receiver, sender)) = IPCClient::create("playit.sock".to_owned())
            else {
                return Err(EngineLocalConnectionError::StartFailed);
            };

            let command_relay = self.start_command_relay(receiver, sender);

            take_mut::take(&mut self.location, |old_engine_location| {
                match old_engine_location {
                    EngineLocation::Invalid => {}
                    EngineLocation::Internal {
                        ipc_server: _,
                        command_processor,
                    } => {
                        command_processor.abort();
                    }
                    EngineLocation::Local {
                        ipc_client: _,
                        command_relay,
                    }
                    | EngineLocation::Remote {
                        ipc_client: _,
                        command_relay,
                    } => {
                        command_relay.abort();
                    }
                };

                EngineLocation::Local {
                    ipc_client,
                    command_relay,
                }
            });

            return Ok(());
        };

        let command_processor = self.start_command_processor(receiver, sender);

        take_mut::take(&mut self.location, |old_engine_location| {
            match old_engine_location {
                EngineLocation::Invalid => {}
                EngineLocation::Internal {
                    ipc_server: _,
                    command_processor,
                } => {
                    command_processor.abort();
                }
                EngineLocation::Local {
                    ipc_client: _,
                    command_relay,
                }
                | EngineLocation::Remote {
                    ipc_client: _,
                    command_relay,
                } => {
                    command_relay.abort();
                }
            };

            EngineLocation::Internal {
                ipc_server,
                command_processor,
            }
        });

        Ok(())
    }

    pub async fn connect_to_remote(
        &mut self,
        address: String,
    ) -> Result<(), EngineRemoteConnectionError> {
        let Ok((new_ipc_client, receiver, sender)) = IPCClient::create(address) else {
            return Err(EngineRemoteConnectionError::ConnectionFailed);
        };

        let command_relay = self.start_command_relay(receiver, sender);

        take_mut::take(&mut self.location, |old_engine_location| {
            match old_engine_location {
                EngineLocation::Invalid => {}
                EngineLocation::Internal {
                    ipc_server: _,
                    command_processor,
                } => {
                    command_processor.abort();
                }
                EngineLocation::Local {
                    ipc_client: _,
                    command_relay,
                }
                | EngineLocation::Remote {
                    ipc_client: _,
                    command_relay,
                } => {
                    command_relay.abort();
                }
            };

            EngineLocation::Remote {
                ipc_client: new_ipc_client,
                command_relay,
            }
        });

        Ok(())
    }

    pub fn connection_status(&self) -> EngineConnectionStatus {
        match &self.location {
            EngineLocation::Invalid => EngineConnectionStatus::Disconnected,
            EngineLocation::Internal {
                ipc_server: _,
                command_processor: _,
            } => EngineConnectionStatus::ConnectedLocal,
            EngineLocation::Local {
                ipc_client: _,
                command_relay: _,
            } => EngineConnectionStatus::ConnectedLocal,
            EngineLocation::Remote {
                ipc_client: _,
                command_relay: _,
            } => EngineConnectionStatus::ConnectedRemote,
        }
    }
}

fn route_response(
    internal: bool,
    internal_sender: &broadcast::Sender<EngineResponse>,
    remote_sender: &broadcast::Sender<(EngineResponse, Uuid)>,
    response: EngineResponse,
    uuid: Uuid,
) {
    if internal {
        let _ = internal_sender.send(response);
    } else if uuid == Uuid::nil() {
        let _ = internal_sender.send(response.clone());
        let _ = remote_sender.send((response, uuid));
    } else {
        let _ = remote_sender.send((response, uuid));
    };
}

fn permission_exists(permission_array: &Vec<Permission>, permission: Permission) -> bool {
    if permission_array.iter().any(|e| *e == permission) {
        true
    } else {
        false
    }
}
