use musicbrainz_rs::entity::recording::Recording;
use serde::{Deserialize, Serialize};

pub mod database;
pub mod sequencer;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RecordingMetadata {
    pub audio_file_hash: Option<String>,

    pub recording: Recording,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PlaylistMetadata {
    pub id: String,

    pub name: String,

    pub recordings: Vec<String>,
}
