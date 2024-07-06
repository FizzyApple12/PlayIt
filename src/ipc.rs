use interprocess::local_socket::{
    tokio::{prelude::*, Stream},
    GenericNamespaced, ListenerOptions,
};
use std::convert::From;
use std::io;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    try_join,
};

#[derive(Debug)]
#[repr(i32)]
#[derive(PartialEq)]
pub enum IPCCommand {
    None = 0,
    Goodbye = 1,

    // Queued Command
    Play = 2,
    Pause = 3,

    // Async Commands
    Status = 4,
    SongMeta = 5,
}

impl From<i32> for IPCCommand {
    fn from(value: i32) -> IPCCommand {
        match value {
            x if x == IPCCommand::Play as i32 => IPCCommand::Play,
            x if x == IPCCommand::Goodbye as i32 => IPCCommand::Goodbye,
            x if x == IPCCommand::Pause as i32 => IPCCommand::Pause,
            x if x == IPCCommand::Status as i32 => IPCCommand::Status,
            x if x == IPCCommand::SongMeta as i32 => IPCCommand::SongMeta,
            _ => IPCCommand::None,
        }
    }
}

pub async fn start(ipc_handler: fn(IPCCommand, Vec<String>)) -> io::Result<()> {
    let socket_name = "playit.sock";
    let socket_ns_name = socket_name.to_ns_name::<GenericNamespaced>()?;

    let listener_options = ListenerOptions::new().name(socket_ns_name);

    let listener = match listener_options.create_tokio() {
        Err(e) if e.kind() == io::ErrorKind::AddrInUse => {
            eprintln!(
                "
Error: could not start server because the socket file is occupied. Please check if {socket_name}
is in use by another process and try again."
            );
            return Err(e.into());
        }
        x => x?,
    };

    println!("Server running at {socket_name}");

    tokio::spawn(async move {
        loop {
            let connection = match listener.accept().await {
                Ok(x) => x,
                Err(e) => {
                    eprintln!("There was an error with an incoming connection: {e}");
                    continue;
                }
            };

            tokio::spawn(async move {
                let mut receiver = BufReader::new(&connection);
                let mut sender = &connection;

                loop {
                    match parse_next(&mut receiver).await {
                        Ok((command_type, args)) => match command_type {
                            IPCCommand::None => {
                                let _ = sender.write_all(b"0");
                            }
                            IPCCommand::Goodbye => {
                                break;
                            }
                            other_command => {
                                ipc_handler(other_command, args);

                                let _ = sender.write_all(b"1");
                            }
                        },
                        Err(e) => {
                            eprintln!("Error while handling connection: {e}");

                            break;
                        }
                    };
                }
            });
        }
    });

    return Ok(());
}

async fn parse_next(receiver: &mut BufReader<&Stream>) -> io::Result<(IPCCommand, Vec<String>)> {
    let mut command_buffer: Vec<String> = Vec::new();

    let mut buffer: String = String::new();

    let readline = receiver.read_line(&mut buffer);
    try_join!(readline)?;

    if buffer.is_empty() {
        return Ok((IPCCommand::Goodbye, command_buffer));
    }

    match buffer.trim_ascii().parse::<i32>() {
        Ok(command_number) => {
            buffer.clear();
            let command_type = command_number.into();

            match command_type {
                IPCCommand::None => {}    // No Args,
                IPCCommand::Goodbye => {} // No Args,
                IPCCommand::Play => {
                    // Song Hash
                    let readline = receiver.read_line(&mut buffer);
                    try_join!(readline)?;
                    command_buffer.push(buffer.trim_ascii().to_string());
                    buffer.clear();
                }
                IPCCommand::Pause => {}  // No Args
                IPCCommand::Status => {} // No Args
                IPCCommand::SongMeta => {
                    // Song Hash
                    let readline = receiver.read_line(&mut buffer);
                    try_join!(readline)?;
                    command_buffer.push(buffer.trim_ascii().to_string());
                    buffer.clear();
                }
            }

            return Ok((command_type, command_buffer));
        }
        Err(e) => {
            eprintln!("Unknown IPC Commnd, {e}");

            return Ok((IPCCommand::None, command_buffer));
        }
    };
}
