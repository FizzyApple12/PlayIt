use std::time::Duration;

use ipc::{client::IPCClient, server::IPCServer};
use player::{database::Database, PlaylistMetadata, SongMetadata};
use rodio::{OutputStream, OutputStreamHandle, Sink};
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
    sink: Sink,
    stream_handle: OutputStreamHandle,

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
    LoopSong,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
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

    SongMetadata(String),
    SongFile(String),
    SendSong((SongMetadata, Vec<u8>)),

    PlaylistMetadata(Uuid),
    SetPlaylistMetadata((Uuid, PlaylistMetadata)),

    SetVolume(f32),

    GetPermissions,
    SetPermissions((Uuid, Vec<Permission>)),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type")]
pub enum EngineResponse {
    Ok(EngineCommand),
    Nope(EngineCommand),

    NowPlaying(String),
    NowPaused(String),

    Seek(Duration),
    CurrentTime(Duration),

    Queue(Vec<String>),

    LoopMode(LoopMode),

    SongMetadata(SongMetadata),
    SongFile((String, Vec<u8>)),

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
    AudioInitializationError,
    DatabaseInitializationError,
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

        let Ok((_stream, stream_handle)) = OutputStream::try_default() else {
            return Err(EngineError::AudioInitializationError);
        };
        let Ok(sink) = Sink::try_new(&stream_handle) else {
            return Err(EngineError::AudioInitializationError);
        };

        let Some(database) = Database::new() else {
            return Err(EngineError::DatabaseInitializationError);
        };

        let mut new_engine = Engine {
            sink,
            stream_handle,
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
        let mut local_command_receiver = self.engine_command_sender.subscribe();
        let local_response_sender = self.engine_response_sender.clone();

        tokio::spawn(async move {
            loop {
                let (command, uuid, local) = tokio::select! {
                    val = local_command_receiver.recv() => {
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
                            local,
                            &local_response_sender,
                            &response_sender,
                            EngineResponse::Ok(command),
                            uuid,
                        );
                    }
                    EngineCommand::Play(_) => {
                        // TODO: Start playing the song if there is one, otherwise resume, otherwise "Nope"
                        route_response(
                            local,
                            &local_response_sender,
                            &response_sender,
                            EngineResponse::NowPlaying("".to_string()),
                            uuid,
                        );
                    }
                    EngineCommand::Pause => {
                        // TODO: Pause the song if there is one, otherwise "Nope"
                        route_response(
                            local,
                            &local_response_sender,
                            &response_sender,
                            EngineResponse::NowPaused("".to_string()),
                            uuid,
                        );
                    }
                    EngineCommand::Next => {
                        // TODO: Play the next song in the queue if there is one and send the new queue, otherwise "Nope"
                        route_response(
                            local,
                            &local_response_sender,
                            &response_sender,
                            EngineResponse::NowPlaying("".to_string()),
                            uuid,
                        );
                        route_response(
                            local,
                            &local_response_sender,
                            &response_sender,
                            EngineResponse::Queue(Vec::new()),
                            uuid,
                        );
                    }
                    EngineCommand::Previous => {
                        // TODO: Play the previous song in the queue if there is one and send the new queue, otherwise "Nope"
                        route_response(
                            local,
                            &local_response_sender,
                            &response_sender,
                            EngineResponse::NowPlaying("".to_string()),
                            uuid,
                        );
                        route_response(
                            local,
                            &local_response_sender,
                            &response_sender,
                            EngineResponse::Queue(Vec::new()),
                            uuid,
                        );
                    }
                    EngineCommand::Seek(_) => {
                        // TODO: Seek to a point in the song if there is one, otherwise "Nope"
                        route_response(
                            local,
                            &local_response_sender,
                            &response_sender,
                            EngineResponse::Seek(Duration::from_secs(0)),
                            uuid,
                        );
                    }
                    EngineCommand::Queue(_) => {
                        // TODO: Add a set of songs to the queue (if songs are missing, a nope will be sent with the missing songs), otherwise get the queue
                        route_response(
                            local,
                            &local_response_sender,
                            &response_sender,
                            EngineResponse::Queue(Vec::new()),
                            uuid,
                        );
                    }
                    EngineCommand::ShuffleQueue(_) => {
                        // TODO: Enable or disable shuffling of the queue
                        route_response(
                            local,
                            &local_response_sender,
                            &response_sender,
                            EngineResponse::Queue(Vec::new()),
                            uuid,
                        );
                    }
                    EngineCommand::ClearQueue => {
                        // TODO: Clear the queue
                        route_response(
                            local,
                            &local_response_sender,
                            &response_sender,
                            EngineResponse::Queue(Vec::new()),
                            uuid,
                        );
                    }
                    EngineCommand::LoopMode(loop_mode) => {
                        // TODO: Set the loop mode
                        route_response(
                            local,
                            &local_response_sender,
                            &response_sender,
                            EngineResponse::LoopMode(loop_mode),
                            uuid,
                        );
                    }
                    EngineCommand::SongMetadata(_) => {
                        // TODO: Get the Song Metadata, otherwise "Nope"
                        route_response(
                            local,
                            &local_response_sender,
                            &response_sender,
                            EngineResponse::SongMetadata(SongMetadata {}),
                            uuid,
                        );
                    }
                    EngineCommand::SongFile(_) => {
                        // TODO: Get the Song File, otherwise "Nope"
                        route_response(
                            local,
                            &local_response_sender,
                            &response_sender,
                            EngineResponse::SongFile(("".to_string(), Vec::new())),
                            uuid,
                        );
                    }
                    EngineCommand::SendSong(_) => {
                        // TODO: Receive the song and database it if user has permissions, otherwise "Nope"
                        route_response(
                            local,
                            &local_response_sender,
                            &response_sender,
                            EngineResponse::Ok(command),
                            uuid,
                        );
                    }
                    EngineCommand::PlaylistMetadata(_) => {
                        // TODO: Get the Song Metadata, otherwise "Nope"
                        route_response(
                            local,
                            &local_response_sender,
                            &response_sender,
                            EngineResponse::PlaylistMetadata(PlaylistMetadata {}),
                            uuid,
                        );
                    }
                    EngineCommand::SetPlaylistMetadata(_) => {
                        // TODO: Set the Playlist Metadata if user has permissions (or is local), otherwise "Nope"
                        route_response(
                            local,
                            &local_response_sender,
                            &response_sender,
                            EngineResponse::Ok(command),
                            uuid,
                        );
                    }
                    EngineCommand::SetVolume(_) => {
                        if local {
                            // TODO: Set volume (ignore this if this was meant for another device)
                        }
                    }
                    EngineCommand::GetPermissions => {
                        // TODO: Get the user's permissions
                        if local {
                            route_response(
                                local,
                                &local_response_sender,
                                &response_sender,
                                EngineResponse::Permissions(Vec::new()),
                                uuid,
                            );

                            let _ = local_response_sender.send(EngineResponse::Permissions(vec![
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
                    EngineCommand::SetPermissions(_) => {
                        if !local {
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

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    resp = response_receiver.recv() => if let Some(resp) = resp {
                        let _ = response_sender.send(resp);
                    },
                    cmd = command_receiver.recv() => if let Ok(cmd) = cmd {
                        if matches!(
                            cmd,
                            EngineCommand::SetVolume(_)
                        ) {
                            // TODO: Set volume (this shouldn't be sent to the remote device)
                        } else {
                            let _ = command_sender.send(cmd);
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
    local: bool,
    local_sender: &broadcast::Sender<EngineResponse>,
    remote_sender: &broadcast::Sender<(EngineResponse, Uuid)>,
    response: EngineResponse,
    uuid: Uuid,
) {
    if local {
        let _ = local_sender.send(response);
    } else {
        let _ = remote_sender.send((response, uuid));
    };
}
