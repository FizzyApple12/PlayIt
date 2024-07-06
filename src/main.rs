mod database;
mod ipc;

use std::{thread, time::Duration};

use interprocess::local_socket::tokio::Stream;
use ipc::IPCCommand;
use tokio::io::AsyncWriteExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = database::create();

    fn handle_ipc_command(
        mut ipc_stream: &Stream,
        ipc_command_type: IPCCommand,
        parameters: Vec<String>,
    ) {
        let _ipc_write = ipc_stream.write_all(b"sample response!");

        println!("IPC Command: {ipc_command_type:?}({parameters:?})");
    }

    let _ = ipc::start(handle_ipc_command).await;

    loop {
        thread::sleep(Duration::from_secs(1));
    }
}
