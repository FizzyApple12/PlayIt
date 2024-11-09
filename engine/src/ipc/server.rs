use interprocess::local_socket::{tokio::prelude::*, GenericNamespaced, ListenerOptions};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter},
    sync::{
        broadcast::{self},
        mpsc::{self},
    },
    task::JoinHandle,
    try_join,
};
use uuid::Uuid;

use crate::{EngineCommand, EngineResponse};

pub enum IPCServerError {
    InvalidAddress,
    AddressInUse,
}

pub struct IPCServer {
    socket_listener: JoinHandle<()>,
}

impl IPCServer {
    pub fn create() -> Result<
        (
            IPCServer,
            mpsc::Receiver<(EngineCommand, Uuid)>,
            broadcast::Sender<(EngineResponse, Uuid)>,
        ),
        IPCServerError,
    > {
        let Ok(socket_ns_name) = "playit.sock".to_ns_name::<GenericNamespaced>() else {
            return Err(IPCServerError::InvalidAddress);
        };

        let listener_options = ListenerOptions::new().name(socket_ns_name);

        let Ok(listener) = listener_options.create_tokio() else {
            return Err(IPCServerError::AddressInUse);
        };

        let (response_sender, _) = broadcast::channel::<(EngineResponse, Uuid)>(16);
        let (command_sender, command_receiver) = mpsc::channel::<(EngineCommand, Uuid)>(16);

        let external_response_sender = response_sender.clone();

        let socket_listener = tokio::spawn(async move {
            loop {
                let connection = match listener.accept().await {
                    Ok(x) => x,
                    Err(_) => {
                        continue;
                    }
                };

                let reader_connection_id = Uuid::new_v4();
                let sender_connection_id = reader_connection_id.clone();

                let new_command_sender = command_sender.clone();

                let (receiver, sender) = connection.split();

                let connection_reader = tokio::spawn(async move {
                    let mut receiver = BufReader::new(receiver);

                    loop {
                        let mut buffer: String = String::new();

                        let readline = receiver.read_line(&mut buffer);

                        if try_join!(readline).is_err() {
                            continue;
                        }

                        if buffer.is_empty() {
                            break;
                        }

                        let Ok(message): Result<EngineCommand, serde_json::Error> =
                            serde_json::from_str(&buffer)
                        else {
                            continue;
                        };

                        match message {
                            EngineCommand::Goodbye => {
                                break;
                            }
                            other_command => {
                                let _ = new_command_sender
                                    .send((other_command, reader_connection_id))
                                    .await;
                            }
                        };
                    }
                });

                let mut new_response_receiver = response_sender.subscribe();

                let connection_writer = tokio::spawn(async move {
                    let mut sender = BufWriter::new(sender);

                    loop {
                        let Ok((response, uuid)) = new_response_receiver.recv().await else {
                            continue;
                        };

                        if uuid != sender_connection_id && !uuid.is_nil() {
                            continue;
                        }

                        let Ok(message): Result<String, serde_json::Error> =
                            serde_json::to_string(&response)
                        else {
                            continue;
                        };

                        let _ = sender.write_all(message.as_bytes()).await;
                    }
                });

                let _ = connection_reader.await;
                connection_writer.abort();
            }
        });

        Ok((
            IPCServer { socket_listener },
            command_receiver,
            external_response_sender,
        ))
    }
}

impl Drop for IPCServer {
    fn drop(&mut self) {
        self.socket_listener.abort();
    }
}
