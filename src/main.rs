mod database;
mod ipc;

use ipc::IPCCommand;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = database::create();

    fn handle_ipc_command(ipc_command_type: IPCCommand, parameters: Vec<String>) {
        println!("IPC Command: {ipc_command_type:?}({parameters:?})");

        return;
    }

    let _ = ipc::start(handle_ipc_command).await;

    loop {}
}
