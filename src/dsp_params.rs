use std::sync::{Arc, Mutex};

#[derive(Clone, Debug)]
pub struct DspParams {
    pub noise_threshold: f32,
    pub freq_shift: f32,
    pub filter_enabled: bool,
    pub gain: f32,
}

impl Default for DspParams {
    fn default() -> Self {
        Self {
            noise_threshold: 0.001,
            freq_shift: 5.0,
            filter_enabled: true,
            gain: 1.0,
        }
    }
}

pub type SharedDspParams = Arc<Mutex<DspParams>>;
