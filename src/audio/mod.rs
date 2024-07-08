use cpal::{traits::{DeviceTrait, HostTrait}, Device, Host, SupportedStreamConfig};

#[derive(PartialEq)]
enum CPALState {
    NotReady = 0,
    HostReady = 1,
    HostDeviceReady = 2,
    HostDeviceConfigReady = 3,
}

pub struct AudioSystem {
    cpal_state: CPALState,
    cpal_host: Host,
    cpal_device: Option<Device>,
    cpal_config: Option<SupportedStreamConfig>
}

pub fn create() -> AudioSystem {
    let cpal_host = cpal::default_host();

    let mut audio_system = AudioSystem {
        cpal_state: CPALState::HostReady,
        cpal_host,
        cpal_device: None,
        cpal_config: None,
    };

    audio_system.prepare();

    return audio_system;
}

impl AudioSystem {
    pub fn prepare(&mut self) -> bool {
        let new_device;
        if self.cpal_state == CPALState::HostReady {
            new_device = self.cpal_host.default_output_device();
        } else {
            new_device = self.cpal_device.clone();
        }

        match new_device {
            Some(device) => {
                self.cpal_device = Some(device);

                if self.cpal_state == CPALState::HostReady {
                    self.cpal_state = CPALState::HostDeviceReady;
                }

                match device.clone().supported_output_configs() {
                    Ok(mut supported_configs_range) => {
                        match supported_configs_range.next() {
                            Some(supported_config) => {
                                self.cpal_config = Some(supported_config.with_max_sample_rate());

                                if self.cpal_state == CPALState::HostDeviceReady {
                                    self.cpal_state = CPALState::HostDeviceConfigReady;
                                }

                                true
                            },
                            None => {
                                self.cpal_state = CPALState::HostDeviceReady;
                                self.cpal_config = None;

                                false
                            }
                        }
                    }
                    Err(_) => {
                        self.cpal_state = CPALState::HostDeviceReady;
                        self.cpal_config = None;

                        false
                    }
                }
            },
            None => {
                self.cpal_state = CPALState::HostReady;
                self.cpal_device = None;

                false
            }
        }
    }
}
