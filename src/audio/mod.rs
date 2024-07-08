use cpal::{
    traits::{DeviceTrait, HostTrait}, Device, Host, Stream, StreamConfig
};

#[derive(PartialEq)]
enum CPALReadyState {
    Host = 1,
    HostDevice = 2,
    HostDeviceConfig = 3,
    HostDeviceConfigStream = 4,
}

pub struct AudioSystem {
    cpal_state: CPALReadyState,
    cpal_host: Host,
    cpal_device: Option<Device>,
    cpal_config: Option<StreamConfig>,
    cpal_stream: Option<Stream>
}

pub fn create() -> AudioSystem {
    let cpal_host = cpal::default_host();

    let mut audio_system = AudioSystem {
        cpal_state: CPALReadyState::Host,
        cpal_host,
        cpal_device: None,
        cpal_config: None,
        cpal_stream: None
    };

    audio_system.prepare();

    audio_system
}

impl AudioSystem {
    pub fn prepare(&mut self) -> bool {
        let new_device = if self.cpal_state == CPALReadyState::Host {
            self.cpal_host.default_output_device()
        } else {
            self.cpal_device.clone()
        };

        match new_device {
            Some(device) => {
                self.cpal_device = Some(device);

                if self.cpal_state == CPALReadyState::Host {
                    self.cpal_state = CPALReadyState::HostDevice;
                }

                let new_config = if self.cpal_state == CPALReadyState::HostDevice {
                    match self
                        .cpal_device
                        .as_ref()
                        .expect(
                            "ummm hi, if you see this, all logic has been defied on the first case",
                        )
                        .supported_output_configs()
                    {
                        Ok(mut supported_configs_range) => {
                            supported_configs_range.next().map(|supported_config| {
                                supported_config.with_max_sample_rate().config()
                            })
                        }
                        Err(_) => None,
                    }
                } else {
                    self.cpal_config.clone()
                };

                match new_config {
                    Some(config) => {
                        self.cpal_config = Some(config);

                        if self.cpal_state == CPALReadyState::HostDevice {
                            self.cpal_state = CPALReadyState::HostDeviceConfig;
                        }

                        let new_stream = if self.cpal_state == CPALReadyState::HostDeviceConfig {
                            match self.cpal_device.as_ref().expect("ummm hi, if you see this, all logic has been defied on the second case").build_output_stream(
                                self.cpal_config.as_ref().expect("ummm hi, if you see this, all logic has been defied on the third case"),
                                |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                                    AudioSystem::stream_data_callback(self, );
                                },
                                move |err| {
                                    // react to errors here.
                                },
                                None // None=blocking, Some(Duration)=timeout
                            ) {
                                Ok(stream) => Some(stream),
                                Err(_) => None,
                            }
                        } else {
                            self.cpal_stream
                        };

                        match new_stream {
                            Some(stream) => {
                                self.cpal_stream = Some(stream);

                                if self.cpal_state == CPALReadyState::HostDeviceConfig {
                                    self.cpal_state = CPALReadyState::HostDeviceConfigStream;
                                }

                                true
                            }
                            None => {
                                self.cpal_state = CPALReadyState::HostDeviceConfig;
                                self.cpal_config = None;

                                false
                            }
                        }
                    }
                    None => {
                        self.cpal_state = CPALReadyState::HostDevice;
                        self.cpal_config = None;

                        false
                    }
                }
            }
            None => {
                self.cpal_state = CPALReadyState::Host;
                self.cpal_device = None;

                false
            }
        }
    }

    fn stream_data_callback(&mut self) {
        
    }

    fn stream_error_callback(&mut self) {
        
    }
}
