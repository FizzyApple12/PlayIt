use std::{
    fs::{DirBuilder, File},
    io::{BufReader, Write},
    path::PathBuf,
    sync::Arc,
    time::Duration,
};

use lazy_static::lazy_static;
use musicbrainz_rs::{entity::recording::Recording, Fetch};
use sled::Db;
use tokio::{sync::Mutex, time};

use super::{PlaylistMetadata, RecordingMetadata};

lazy_static! {
    static ref root_db_path: PathBuf = PathBuf::from(&shellexpand::tilde("~/.playit/").to_string());
}

pub struct Database {
    metadata_db: Arc<Mutex<Db>>,
    playlist_db: Arc<Mutex<Db>>,
}

pub enum DatabaseError {
    InitializationFailed,
    DatabaseFailure,
    MusicbrainzFailure,
    DataConversionFailure,
    RecordingMetadataNotFound,
    RecordingFileNotFound,
    PlaylistNotFound,
}

impl Database {
    pub fn new() -> Result<Database, DatabaseError> {
        let _ = DirBuilder::new()
            .recursive(true)
            .create(root_db_path.clone().join("audio/"));

        let Ok(raw_metadata_db) = sled::open(root_db_path.clone().join("metadata")) else {
            return Err(DatabaseError::InitializationFailed);
        };
        let Ok(raw_playlist_db) = sled::open(root_db_path.clone().join("playlist")) else {
            return Err(DatabaseError::InitializationFailed);
        };

        let metadata_db = Arc::new(Mutex::new(raw_metadata_db));
        let playlist_db = Arc::new(Mutex::new(raw_playlist_db));

        let metadata_db_copy = metadata_db.clone();
        let playlist_db_copy = playlist_db.clone();

        tokio::spawn(async move {
            loop {
                time::sleep(Duration::from_secs(30)).await;

                let _ = metadata_db_copy.lock().await.flush_async().await;
            }
        });
        tokio::spawn(async move {
            loop {
                time::sleep(Duration::from_secs(30)).await;

                let _ = playlist_db_copy.lock().await.flush_async().await;
            }
        });

        Ok(Database {
            metadata_db,
            playlist_db,
        })
    }

    pub async fn get_recording_file(&self, id: String) -> Result<BufReader<File>, DatabaseError> {
        let Ok(metadata) = self.get_recording_metadata(id.clone()).await else {
            return Err(DatabaseError::RecordingMetadataNotFound);
        };

        let Some(audio_file_hash) = metadata.audio_file_hash.clone() else {
            return Err(DatabaseError::RecordingFileNotFound);
        };

        let Ok(file) = File::open(root_db_path.clone().join("audio/").join(audio_file_hash)) else {
            let _ = self.set_recording_file(id, None);

            return Err(DatabaseError::RecordingFileNotFound);
        };

        Ok(BufReader::new(file))
    }

    pub async fn set_recording_file(&self, id: String, file_contents: Option<Vec<u8>>) {
        let Ok(mut metadata) = self.get_recording_metadata(id.clone()).await else {
            return;
        };

        let Some(file_contents) = file_contents else {
            metadata.audio_file_hash = Option::None;

            if let Ok(metadata_bytes) = serde_json::to_vec(&metadata) {
                let _ = self.metadata_db.lock().await.insert(id, metadata_bytes);
            };

            return;
        };

        let audio_file_hash = sha256::digest(&file_contents);

        let Ok(mut file) = File::create(
            root_db_path
                .clone()
                .join("audio/")
                .join(audio_file_hash.clone()),
        ) else {
            return;
        };

        let _ = file.write_all(&file_contents);

        metadata.audio_file_hash = Some(audio_file_hash);

        let Ok(metadata_bytes): Result<Vec<u8>, serde_json::Error> = serde_json::to_vec(&metadata)
        else {
            return;
        };

        let _ = self.metadata_db.lock().await.insert(id, &*metadata_bytes);
    }

    pub async fn get_recording_metadata(
        &self,
        id: String,
    ) -> Result<RecordingMetadata, DatabaseError> {
        let Ok(contains) = self.metadata_db.lock().await.get(id.clone()) else {
            return Err(DatabaseError::DatabaseFailure);
        };

        let Some(metadata_bytes) = contains else {
            let Ok(recording) = Recording::fetch().id(&id).execute().await else {
                return Err(DatabaseError::MusicbrainzFailure);
            };

            let new_metadata = RecordingMetadata {
                audio_file_hash: Option::None,

                recording,
            };

            let Ok(metadata_bytes): Result<Vec<u8>, serde_json::Error> =
                serde_json::to_vec(&new_metadata)
            else {
                return Err(DatabaseError::DataConversionFailure);
            };

            let _ = self.metadata_db.lock().await.insert(id, &*metadata_bytes);

            return Ok(new_metadata);
        };

        let Ok(metadata): Result<RecordingMetadata, serde_json::Error> =
            serde_json::from_slice(&metadata_bytes)
        else {
            return Err(DatabaseError::DataConversionFailure);
        };

        Ok(metadata)
    }

    pub async fn get_playlist(&self, id: String) -> Result<PlaylistMetadata, DatabaseError> {
        let Ok(contains) = self.playlist_db.lock().await.get(id.clone()) else {
            return Err(DatabaseError::DatabaseFailure);
        };

        let Some(metadata_bytes) = contains else {
            return Err(DatabaseError::PlaylistNotFound);
        };

        let Ok(metadata): Result<PlaylistMetadata, serde_json::Error> =
            serde_json::from_slice(&metadata_bytes)
        else {
            return Err(DatabaseError::DataConversionFailure);
        };

        Ok(metadata)
    }

    pub async fn set_playlist(&self, metadata: PlaylistMetadata) {
        let id = metadata.id.clone();

        let Ok(metadata_bytes): Result<Vec<u8>, serde_json::Error> = serde_json::to_vec(&metadata)
        else {
            return;
        };

        let _ = self.metadata_db.lock().await.insert(id, &*metadata_bytes);
    }
}

impl Clone for Database {
    fn clone(&self) -> Self {
        Self {
            metadata_db: self.metadata_db.clone(),
            playlist_db: self.playlist_db.clone(),
        }
    }
}
