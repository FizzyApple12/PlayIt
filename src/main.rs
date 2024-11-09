use playit_engine::{Engine, EngineCommand};

#[derive(Debug)]
enum PlayItError {
    EngineError,
}

#[tokio::main]
async fn main() -> Result<(), PlayItError> {
    let Ok((mut audio_engine, command_sender, mut command_receiver)) = Engine::create() else {
        return Err(PlayItError::EngineError);
    };

    let _ = audio_engine.connect_to_local();

    let _ = command_sender.send(EngineCommand::RecordingMetadata(
        "e2c2390c-32d3-446d-b904-0b347927165c".to_string(),
    ));

    loop {
        println!("Get Metadata: {:?}", command_receiver.recv().await);
    }
}
