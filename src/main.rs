mod audio;
mod database;
mod ipc;

use std::{thread, time::Duration};

use interprocess::local_socket::tokio::Stream;
use ipc::IPCCommand;
use tokio::io::AsyncWriteExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // let _ = database::create();
    let audio_system = audio::playback::create();

    fn handle_ipc_command(
        mut ipc_stream: &Stream,
        ipc_command_type: IPCCommand,
        parameters: Vec<String>,
    ) {
        let _ipc_write = ipc_stream.write_all(b"sample response!");

        println!("IPC Command: {ipc_command_type:?}({parameters:?})");
    }

    let _ = ipc::start(handle_ipc_command).await;

    let Some(current_config) = audio_system.get_config() else {
        println!("No config available!");

        return Ok(());
    };

    let sample_rate = current_config.sample_rate.0 as f32;
    let channels = current_config.channels as usize;

    let mut sample_clock = 0f32;
    let mut next_sample = move || {
        sample_clock = (sample_clock + 1.0) % sample_rate;
        (sample_clock * 440.0 * 2.0 * std::f32::consts::PI / sample_rate).sin()
    };

    audio_system.play();

    loop {
        for _ in 0..100000 {
            let sample_data = vec![next_sample()];
            
            for _ in 0..channels {
                audio_system.write_next_samples(&sample_data);
            }
        }

        thread::sleep(Duration::from_secs(1));
    }
}
