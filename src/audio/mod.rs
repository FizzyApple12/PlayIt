use std::sync::mpsc::{self, Receiver, Sender};

use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait}, BuildStreamError, Device, FromSample, Host, Sample, SampleFormat, Stream, StreamError, SupportedStreamConfig
};

pub struct AudioSystem {
    cpal_host: Host,
    cpal_device: Option<Device>,
    pub cpal_config: Option<SupportedStreamConfig>,
    cpal_stream: Option<Stream>,

    audio_buffer: Option<Sender<f32>>,
    error_buffer: Option<Receiver<StreamError>>,
}

pub fn create() -> AudioSystem {
    let cpal_host = cpal::default_host();

    let mut audio_system = AudioSystem {
        cpal_host,

        cpal_device: None,
        cpal_config: None,
        cpal_stream: None,

        audio_buffer: None,
        error_buffer: None,
    };

    audio_system.prepare();

    audio_system
}

impl AudioSystem {
    pub fn prepare(&mut self) -> bool {
        // Init or take the existing CPAL Device
        if self.cpal_device.is_none() {
            let Some(device) = self.new_device() else {
                self.cpal_device = None;
                self.cpal_config = None;
                self.cpal_stream = None;

                println!("Failed to open device!");

                return false;
            };

            self.cpal_device = Some(device);
            self.cpal_config = None;
            self.cpal_stream = None;
        }

        // Init or take the existing CPAL Config
        if self.cpal_config.is_none() {
            let Some(config) = self.new_config() else {
                self.cpal_config = None;
                self.cpal_stream = None;

                println!("Failed to get config!");

                return false;
            };

            self.cpal_config = Some(config);
            self.cpal_stream = None;
        }

        // Init or take the existing CPAL Stream
        if self.cpal_stream.is_none() {
            let (audio_sender, audio_receiver) = mpsc::channel();
            let (error_sender, error_receiver) = mpsc::channel();

            let Some(stream) = self.new_stream(audio_receiver, error_sender) else {
                self.cpal_stream = None;

                println!("Failed to init stream!");

                return false;
            };

            self.cpal_stream = Some(stream);
            self.audio_buffer = Some(audio_sender);
            self.error_buffer = Some(error_receiver);
        }

        true
    }

    fn new_device(&self) -> Option<Device> {
        self.cpal_host.default_output_device()
    }

    fn new_config(&self) -> Option<SupportedStreamConfig> {
        let device = self
            .cpal_device
            .as_ref()
            .expect("ummm hi, if you see this, all logic has been defied on the first case");

        match device.supported_output_configs() {
            Ok(mut supported_configs_range) => supported_configs_range
                .next()
                .map(|supported_config| supported_config.with_max_sample_rate()),
            Err(_) => None,
        }
    }

    fn new_stream(&self, audio_receiver: Receiver<f32>, error_sender: Sender<StreamError>) -> Option<Stream> {
        let config = self
            .cpal_config
            .as_ref()
            .expect("ummm hi, if you see this, all logic has been defied on the second case");

        let device = self
            .cpal_device
            .as_ref()
            .expect("ummm hi, if you see this, all logic has been defied on the third case");

        match config.sample_format() {
            SampleFormat::F32 => device.build_output_stream(
                &config.config(),
                move |data, info| {
                    Self::stream_data_callback::<f32>(data, info, &audio_receiver)
                },
                move |error| {
                    let _ = error_sender.send(error);
                },
                None,
            ),
            SampleFormat::I16 => device.build_output_stream(
                &config.config(),
                move |data, info| {
                    Self::stream_data_callback::<i16>(data, info, &audio_receiver)
                },
                move |error| {
                    let _ = error_sender.send(error);
                },
                None,
            ),
            SampleFormat::U16 => device.build_output_stream(
                &config.config(),
                move |data, info| {
                    Self::stream_data_callback::<u16>(data, info, &audio_receiver)
                },
                move |error| {
                    let _ = error_sender.send(error);
                },
                None,
            ),
            SampleFormat::I8 => device.build_output_stream(
                &config.config(),
                move |data, info| {
                    Self::stream_data_callback::<i8>(data, info, &audio_receiver)
                },
                move |error| {
                    let _ = error_sender.send(error);
                },
                None,
            ),
            SampleFormat::I32 => device.build_output_stream(
                &config.config(),
                move |data, info| {
                    Self::stream_data_callback::<i32>(data, info, &audio_receiver)
                },
                move |error| {
                    let _ = error_sender.send(error);
                },
                None,
            ),
            SampleFormat::I64 => device.build_output_stream(
                &config.config(),
                move |data, info| {
                    Self::stream_data_callback::<i64>(data, info, &audio_receiver)
                },
                move |error| {
                    let _ = error_sender.send(error);
                },
                None,
            ),
            SampleFormat::U8 => device.build_output_stream(
                &config.config(),
                move |data, info| {
                    Self::stream_data_callback::<u8>(data, info, &audio_receiver)
                },
                move |error| {
                    let _ = error_sender.send(error);
                },
                None,
            ),
            SampleFormat::U32 => device.build_output_stream(
                &config.config(),
                move |data, info| {
                    Self::stream_data_callback::<u32>(data, info, &audio_receiver)
                },
                move |error| {
                    let _ = error_sender.send(error);
                },
                None,
            ),
            SampleFormat::U64 => device.build_output_stream(
                &config.config(),
                move |data, info| {
                    Self::stream_data_callback::<u64>(data, info, &audio_receiver)
                },
                move |error| {
                    let _ = error_sender.send(error);
                },
                None,
            ),
            SampleFormat::F64 => device.build_output_stream(
                &config.config(),
                move |data, info| {
                    Self::stream_data_callback::<f64>(data, info, &audio_receiver)
                },
                move |error| {
                    let _ = error_sender.send(error);
                },
                None,
            ),
            _ => {
                Err(BuildStreamError::StreamConfigNotSupported)
            },
        }
        .ok()
    }

    fn stream_data_callback<T: Sample + FromSample<f32>>(
        data: &mut [T],
        _output_callback_info: &cpal::OutputCallbackInfo,
        audio_buffer_reference: &Receiver<f32>
    ) {
        for sample in data.iter_mut() {
            match audio_buffer_reference.try_recv() {
                Ok(sample_value) => *sample = T::from_sample(sample_value),
                Err(_) => *sample = Sample::EQUILIBRIUM,
            }
        }
    }

    pub fn write_next_samples(&self, new_samples: &[f32]) -> bool {
        match &self.audio_buffer {
            Some(buffer) => {
                for sample in new_samples.iter() {
                    let _ = buffer.send(*sample);
                }
                true
            },
            None => false
        }
    }

    pub fn play(&self) -> bool {
        let Some(ref stream) = self.cpal_stream else {
            return false;
        };

        stream.play().is_ok()
    }

    pub fn pause(&self) -> bool {
        let Some(ref stream) = self.cpal_stream else {
            return false;
        };

        stream.pause().is_ok()
    }
}
