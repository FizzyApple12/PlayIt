use serde::{Deserialize, Serialize};

pub mod database;
pub mod sequencer;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SongMetadata {}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PlaylistMetadata {}
