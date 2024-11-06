use std::io::{BufRead, BufReader, BufWriter, Write};

use interprocess::local_socket::{
    tokio::prelude::*, traits::Stream as StreamTrait, GenericNamespaced, Stream,
};
use tokio::{sync::mpsc, task::JoinHandle};

use crate::{EngineCommand, EngineResponse};

pub enum IPCClientError {
    InvalidAddress,
    ConnectionFailed,
}

pub struct IPCClient {
    commection_reader: JoinHandle<()>,
    commection_writer: JoinHandle<()>,
    internal_command_sender: mpsc::Sender<EngineCommand>,
}

impl IPCClient {
    pub fn create(
        address: String,
    ) -> Result<
        (
            IPCClient,
            mpsc::Receiver<EngineResponse>,
            mpsc::Sender<EngineCommand>,
        ),
        IPCClientError,
    > {
        let Ok(socket_ns_name) = address.to_ns_name::<GenericNamespaced>() else {
            return Err(IPCClientError::InvalidAddress);
        };

        let Ok(stream) = Stream::connect(socket_ns_name) else {
            return Err(IPCClientError::ConnectionFailed);
        };

        let (response_sender, response_receiver) = mpsc::channel::<EngineResponse>(16);
        let (command_sender, mut command_receiver) = mpsc::channel::<EngineCommand>(16);

        let internal_command_sender = command_sender.clone();

        let (receiver, sender) = stream.split();

        let commection_reader = tokio::spawn(async move {
            let mut receiver = BufReader::new(receiver);

            loop {
                let mut buffer: String = String::new();

                let Ok(_) = receiver.read_line(&mut buffer) else {
                    continue;
                };

                if buffer.is_empty() {
                    break;
                }

                let Ok(message): Result<EngineResponse, serde_json::Error> =
                    serde_json::from_str(&buffer)
                else {
                    continue;
                };

                let _ = response_sender.send(message);
            }
        });

        let commection_writer = tokio::spawn(async move {
            let mut sender = BufWriter::new(sender);

            loop {
                let Some(command) = command_receiver.recv().await else {
                    continue;
                };

                let Ok(message): Result<String, serde_json::Error> =
                    serde_json::to_string(&command)
                else {
                    continue;
                };

                let _ = sender.write_all(message.as_bytes());
            }
        });

        Ok((
            IPCClient {
                commection_reader,
                commection_writer,
                internal_command_sender,
            },
            response_receiver,
            command_sender,
        ))
    }
}

impl Drop for IPCClient {
    fn drop(&mut self) {
        let _ = self.internal_command_sender.send(EngineCommand::Goodbye);

        self.commection_reader.abort();
        self.commection_writer.abort();
    }
}
