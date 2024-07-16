use std::sync::mpsc::{self, Receiver, Sender};

use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    BuildStreamError, Device, FromSample, Host, Sample, SampleFormat, Stream, StreamConfig,
    StreamError, SupportedStreamConfig,
};

#[macro_export]
macro_rules! create_output_stream {
    ($device:tt, $config:tt, $x:ty, $audio_receiver:tt, $error_sender:tt) => {
        $device.build_output_stream(
            &$config.config(),
            move |data, info| Self::stream_data_callback::<$x>(data, info, &$audio_receiver),
            move |error| {
                let _ = $error_sender.send(error);
            },
            None,
        )
    };
}

pub struct AudioPlayback {
    cpal_host: Host,
    cpal_device: Option<Device>,
    cpal_config: Option<SupportedStreamConfig>,
    cpal_stream: Option<Stream>,

    audio_buffer: Option<Sender<f32>>,
    error_buffer: Option<Receiver<StreamError>>,
}

pub fn create() -> AudioPlayback {
    let cpal_host = cpal::default_host();

    let mut audio_system = AudioPlayback {
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

impl AudioPlayback {
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

    fn new_stream(
        &self,
        audio_receiver: Receiver<f32>,
        error_sender: Sender<StreamError>,
    ) -> Option<Stream> {
        let config = self
            .cpal_config
            .as_ref()
            .expect("ummm hi, if you see this, all logic has been defied on the second case");

        let device = self
            .cpal_device
            .as_ref()
            .expect("ummm hi, if you see this, all logic has been defied on the third case");

        match config.sample_format() {
            SampleFormat::F32 => create_output_stream!(device, config, f32, audio_receiver, error_sender),
            SampleFormat::I16 => create_output_stream!(device, config, i16, audio_receiver, error_sender),
            SampleFormat::U16 => create_output_stream!(device, config, u16, audio_receiver, error_sender),
            SampleFormat::I8 => create_output_stream!(device, config, i8, audio_receiver, error_sender),
            SampleFormat::I32 => create_output_stream!(device, config, i32, audio_receiver, error_sender),
            SampleFormat::I64 => create_output_stream!(device, config, i64, audio_receiver, error_sender),
            SampleFormat::U8 => create_output_stream!(device, config, u8, audio_receiver, error_sender),
            SampleFormat::U32 => create_output_stream!(device, config, u32, audio_receiver, error_sender),
            SampleFormat::U64 => create_output_stream!(device, config, u64, audio_receiver, error_sender),
            SampleFormat::F64 => create_output_stream!(device, config, f64, audio_receiver, error_sender),
            _ => Err(BuildStreamError::StreamConfigNotSupported),
        }
        .ok()
    }

    fn stream_data_callback<T: Sample + FromSample<f32>>(
        data: &mut [T],
        _output_callback_info: &cpal::OutputCallbackInfo,
        audio_buffer_reference: &Receiver<f32>,
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
            }
            None => false,
        }
    }

    pub fn get_config(&self) -> Option<StreamConfig> {
        Some(self.cpal_config.as_ref()?.config())
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
