use std::{
    fs::{read_dir, File},
    io::BufReader,
    path::PathBuf,
};

use serde::{Deserialize, Serialize};

pub struct Database {
    root_db_path: PathBuf,
    available_audio_files: Vec<String>,
    // available_metadata: Vec<SongMetadata>
}

impl Database {
    pub fn new() -> Option<Database> {
        let root_db_path: PathBuf = PathBuf::from(&shellexpand::tilde("~/.playit").to_string());

        let Ok(audio_directory_iterator) = read_dir(root_db_path.join("audio")) else {
            return None;
        };

        let mut all_audio_files: Vec<String> = Vec::new();

        for audio_file in audio_directory_iterator {
            let Ok(entry) = audio_file else {
                continue;
            };

            let path = entry.path();

            let Some(file_stem) = path.file_stem() else {
                continue;
            };

            let Some(file_stem_string) = file_stem.to_str() else {
                continue;
            };

            all_audio_files.push(file_stem_string.to_string());
        }

        Some(Database {
            root_db_path,
            available_audio_files: all_audio_files,
        })
    }

    pub fn get_audio(self, hash: String) -> Option<BufReader<File>> {
        let Ok(file) = File::open(self.root_db_path.join("audio").join(hash + ".flac")) else {
            return None;
        };

        Some(BufReader::new(file))
    }

    // pub fn get_metadata
}
