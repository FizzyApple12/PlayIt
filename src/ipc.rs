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
    fn from(v: i32) -> IPCCommand {
        match v {
            x if x == IPCCommand::Play as i32 => IPCCommand::Play,
            x if x == IPCCommand::Pause as i32 => IPCCommand::Pause,
            x if x == IPCCommand::Status as i32 => IPCCommand::Status,
            x if x == IPCCommand::SongMeta as i32 => IPCCommand::SongMeta,
            _ => IPCCommand::None,
        }
    }
}

pub async fn start(ipc_handler: fn(IPCCommand, Vec<String>)) -> io::Result<()> {
    let printname = "playit.sock";
    let name = printname.to_ns_name::<GenericNamespaced>()?;

    let opts = ListenerOptions::new().name(name);

    let listener = match opts.create_tokio() {
        Err(e) if e.kind() == io::ErrorKind::AddrInUse => {
            eprintln!(
                "
Error: could not start server because the socket file is occupied. Please check if {printname}
is in use by another process and try again."
            );
            return Err(e.into());
        }
        x => x?,
    };

    println!("Server running at {printname}");

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
                                let _ = sender.write_all(b"0");

                                return;
                            }
                            other_command => {
                                ipc_handler(other_command, args);

                                let _ = sender.write_all(b"1");
                            }
                        },
                        Err(e) => {
                            eprintln!("Error while handling connection: {e}");

                            let _ = sender.write_all(b"0");

                            return;
                        }
                    }
                }
            });
        }
    });

    return Ok(());
}

async fn parse_next(receiver: &mut BufReader<&Stream>) -> io::Result<(IPCCommand, Vec<String>)> {
    let mut command_buffer: Vec<String> = Vec::new();

    let mut buffer: String = String::new();

    let recv = receiver.read_line(&mut buffer);
    try_join!(recv)?;

    if buffer.is_empty() {
        return Ok((IPCCommand::Goodbye, command_buffer));
    }

    match buffer.trim_ascii().parse::<i32>() {
        Ok(command_number) => {
            buffer.clear();
            let command_type = command_number.into();

            match command_type {
                IPCCommand::None => return Ok((IPCCommand::None, command_buffer)),
                IPCCommand::Goodbye => return Ok((IPCCommand::Goodbye, command_buffer)),
                IPCCommand::Play => {
                    let recv = receiver.read_line(&mut buffer); // Song Hash
                    try_join!(recv)?;
                    command_buffer.push(buffer.trim_ascii().to_string());
                    buffer.clear();
                }
                IPCCommand::Pause => {}  // No Args
                IPCCommand::Status => {} // No Args
                IPCCommand::SongMeta => {
                    let recv = receiver.read_line(&mut buffer); // Song Hash
                    try_join!(recv)?;
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
