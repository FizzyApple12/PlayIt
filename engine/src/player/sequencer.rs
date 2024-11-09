use std::{sync::Arc, time::Duration};

use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink, Source};
use tokio::sync::Mutex;

use crate::LoopMode;

use super::database::Database;

pub struct Sequencer {
    sink: Arc<Mutex<Sink>>,
    stream_handle: Arc<Mutex<OutputStreamHandle>>,

    playing: Arc<Mutex<Option<String>>>,
    loop_mode: Arc<Mutex<LoopMode>>,
    shuffle: Arc<Mutex<bool>>,

    queue: Arc<Mutex<Vec<String>>>,
    shuffled_queue: Arc<Mutex<Vec<String>>>,

    song_backlog: Arc<Mutex<Vec<String>>>,

    database: Database,
}

pub enum SequencerError {
    AudioInitializationFailed,
    MissingAudioFile,
    DecodingError,
    SeekFailed,
    NothingPlaying,
    NoSongsPlayed,
    NoSongsQueued,
}

impl Sequencer {
    pub fn new(database: Database) -> Result<Sequencer, SequencerError> {
        let Ok((_stream, stream_handle)) = OutputStream::try_default() else {
            return Err(SequencerError::AudioInitializationFailed);
        };
        let Ok(sink) = Sink::try_new(&stream_handle) else {
            return Err(SequencerError::AudioInitializationFailed);
        };

        sink.pause();

        Ok(Sequencer {
            sink: Arc::new(Mutex::new(sink)),
            stream_handle: Arc::new(Mutex::new(stream_handle)),

            playing: Arc::new(Mutex::new(None)),
            loop_mode: Arc::new(Mutex::new(LoopMode::None)),
            shuffle: Arc::new(Mutex::new(false)),

            queue: Arc::new(Mutex::new(Vec::new())),
            shuffled_queue: Arc::new(Mutex::new(Vec::new())),

            song_backlog: Arc::new(Mutex::new(Vec::new())),

            database,
        })
    }

    pub async fn get_playing(&self) -> Option<String> {
        let locked_sink = self.sink.lock().await;

        if locked_sink.is_paused() {
            return None;
        }

        self.playing.lock().await.clone()
    }

    pub async fn play(&self, id: String) -> Result<(), SequencerError> {
        let Ok(file) = self.database.get_recording_file(id.clone()).await else {
            return Err(SequencerError::MissingAudioFile);
        };

        let Ok(decoded_file) = Decoder::new(file) else {
            return Err(SequencerError::DecodingError);
        };

        let locked_sink = self.sink.lock().await;
        locked_sink.append(decoded_file.convert_samples::<f32>());
        locked_sink.play();

        *self.playing.lock().await = Some(id);

        Ok(())
    }

    pub async fn pause(&self) {
        self.sink.lock().await.pause();
    }

    pub async fn seek(&self, position: Duration) -> Result<(), SequencerError> {
        if self.sink.lock().await.try_seek(position).is_err() {
            Err(SequencerError::SeekFailed)
        } else {
            Ok(())
        }
    }

    pub async fn next(&self) -> Result<(), SequencerError> {
        match *self.loop_mode.lock().await {
            LoopMode::None => {
                let should_shuffle = *self.shuffle.lock().await;

                let mut locked_queue = if should_shuffle {
                    self.shuffled_queue.lock().await
                } else {
                    self.queue.lock().await
                };

                if locked_queue.len() == 0 {
                    return Err(SequencerError::NoSongsQueued);
                }

                let song_to_play = locked_queue.remove(0);

                if should_shuffle {
                    let mut locked_removal_queue = self.queue.lock().await;

                    if let Some(song_index) = locked_removal_queue
                        .iter()
                        .position(|song_to_check| *song_to_check == song_to_play)
                    {
                        locked_removal_queue.remove(song_index);
                    }
                }

                self.play(song_to_play).await?;

                Ok(())
            }
            LoopMode::LoopQueue => {
                let should_shuffle = *self.shuffle.lock().await;

                if should_shuffle {
                    let mut locked_shuffle_queue = self.shuffled_queue.lock().await;

                    if locked_shuffle_queue.len() == 0 {
                        let locked_queue = self.queue.lock().await;

                        if locked_queue.len() == 0 {
                            return Err(SequencerError::NoSongsQueued);
                        }

                        *self.shuffled_queue.lock().await =
                            shuffle_queue(self.queue.lock().await.to_vec());
                    }

                    let song_to_play = locked_shuffle_queue.remove(0);

                    self.play(song_to_play).await?;

                    Ok(())
                } else {
                    let mut locked_queue = self.queue.lock().await;

                    if locked_queue.len() == 0 {
                        return Err(SequencerError::NoSongsQueued);
                    }

                    let song_to_play = locked_queue.remove(0);

                    locked_queue.push(song_to_play.clone());

                    self.play(song_to_play).await?;

                    Ok(())
                }
            }
            LoopMode::LoopRecording => {
                let Some(ref song_to_loop) = *self.playing.lock().await else {
                    return Err(SequencerError::NothingPlaying);
                };

                self.play(song_to_loop.clone()).await?;

                Ok(())
            }
        }
    }

    pub async fn previous(&self) -> Result<(), SequencerError> {
        let mut locked_backlog = self.song_backlog.lock().await;

        if locked_backlog.len() == 0 {
            return Err(SequencerError::NoSongsPlayed);
        }

        let song_to_play = locked_backlog.remove(0);

        self.queue.lock().await.insert(0, song_to_play.clone());

        if *self.shuffle.lock().await {
            self.shuffled_queue
                .lock()
                .await
                .insert(0, song_to_play.clone());
        }

        return self.play(song_to_play).await;
    }

    pub async fn add_queue(&self, ids: Vec<String>) -> Result<Vec<String>, SequencerError> {
        let mut unplayable = Vec::new();

        let mut locked_queue = self.queue.lock().await;

        for id in ids {
            if self.database.get_recording_file(id.clone()).await.is_ok() {
                locked_queue.push(id);
            } else {
                unplayable.push(id);
            }
        }

        if *self.shuffle.lock().await {
            *self.shuffled_queue.lock().await = shuffle_queue(self.queue.lock().await.to_vec());
        }

        return Ok(unplayable);
    }

    pub async fn get_queue(&self) -> Vec<String> {
        if *self.shuffle.lock().await {
            self.shuffled_queue.lock().await.clone()
        } else {
            self.queue.lock().await.clone()
        }
    }

    pub async fn clear_queue(&self) {
        self.queue.lock().await.clear();
        self.shuffled_queue.lock().await.clear();
    }

    pub async fn set_loop_mode(&self, mode: LoopMode) {
        *self.loop_mode.lock().await = mode;
    }

    pub async fn set_shuffle(&self, enable: bool) {
        if enable {
            *self.shuffled_queue.lock().await = shuffle_queue(self.queue.lock().await.to_vec());
        } else {
            self.shuffled_queue.lock().await.clear();
        }

        *self.shuffle.lock().await = enable;
    }

    pub async fn set_volume(&self, volume: f32) {
        self.sink.lock().await.set_volume(volume);
    }
}

impl Clone for Sequencer {
    fn clone(&self) -> Self {
        Self {
            sink: self.sink.clone(),
            stream_handle: self.stream_handle.clone(),
            playing: self.playing.clone(),
            loop_mode: self.loop_mode.clone(),
            shuffle: self.shuffle.clone(),
            queue: self.queue.clone(),
            shuffled_queue: self.queue.clone(),
            song_backlog: self.song_backlog.clone(),
            database: self.database.clone(),
        }
    }
}

fn shuffle_queue(queue: Vec<String>) -> Vec<String> {
    let mut shuffle_array = queue;

    for i in 0..(shuffle_array.len() - 2) {
        let j = (rand::random::<u32>() as usize % (shuffle_array.len() - i)) + i;

        shuffle_array.swap(i, j);
    }

    shuffle_array
}
