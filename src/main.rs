use std::{thread, time::Duration};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    loop {
        thread::sleep(Duration::from_secs(1));
    }
}
