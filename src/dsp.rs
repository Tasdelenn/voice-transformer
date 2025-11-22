use anyhow::Result;
use std::f32::consts::PI;
use crate::dsp_params::SharedDspParams;

struct Biquad {
    a0: f32, a1: f32, a2: f32,
    b1: f32, b2: f32,
    z1: f32, z2: f32,
}

impl Biquad {
    fn new_bandpass(sample_rate: f32, center_freq: f32, q: f32) -> Self {
        let w0 = 2.0 * PI * center_freq / sample_rate;
        let alpha = w0.sin() / (2.0 * q);
        
        let b0 = alpha;
        let b1 = 0.0;
        let b2 = -alpha;
        let a0 = 1.0 + alpha;
        let a1 = -2.0 * w0.cos();
        let a2 = 1.0 - alpha;

        Self {
            a0: b0 / a0, a1: b1 / a0, a2: b2 / a0,
            b1: a1 / a0, b2: a2 / a0,
            z1: 0.0, z2: 0.0,
        }
    }

    fn process(&mut self, input: f32) -> f32 {
        let output = input * self.a0 + self.z1;
        self.z1 = input * self.a1 + self.z2 - self.b1 * output;
        self.z2 = input * self.a2 - self.b2 * output;
        output
    }
}

pub struct DspProcessor {
    params: SharedDspParams,
    // Filters
    hp_filter: Biquad, // High Pass (300Hz)
    lp_filter: Biquad, // Low Pass (3400Hz)
    // Frequency shift state
    phase: f32,
    sample_rate: f32,
}

impl DspProcessor {
    pub fn new(sample_rate: u32, _channels: u16, params: SharedDspParams) -> Result<Self> {
        let sr = sample_rate as f32;
        
        // Human voice range: ~300Hz to ~3400Hz (Telephony standard)
        // High Pass at 300Hz (Q=0.707)
        let hp_filter = Biquad::new_highpass(sr, 300.0, 0.707);
        // Low Pass at 3400Hz (Q=0.707)
        let lp_filter = Biquad::new_lowpass(sr, 3400.0, 0.707);

        Ok(Self {
            params,
            hp_filter,
            lp_filter,
            phase: 0.0,
            sample_rate: sr,
        })
    }

    pub fn process_frame(&mut self, frame: &mut [f32]) -> Result<()> {
        // Lock params once per frame to avoid contention
        let params = {
            let p = self.params.lock().unwrap();
            p.clone()
        };

        for sample in frame.iter_mut() {
            let mut s = *sample;

            // 1. Band-Pass Filter (if enabled)
            if params.filter_enabled {
                s = self.hp_filter.process(s);
                s = self.lp_filter.process(s);
            }

            // 2. Frequency Shift
            self.phase += 2.0 * PI * params.freq_shift / self.sample_rate;
            if self.phase >= 2.0 * PI {
                self.phase -= 2.0 * PI;
            }
            s = s * (1.0 + 0.05 * self.phase.sin());

            // 3. Noise Gate & Gain
            if s.abs() < params.noise_threshold {
                s = 0.0;
            } else {
                s = s * params.gain;
            }

            *sample = s;
        }
        Ok(())
    }
    
    pub fn process_render_frame(&mut self, _frame: &mut [f32]) -> Result<()> {
        Ok(())
    }
}

impl Biquad {
    fn new_highpass(sample_rate: f32, freq: f32, q: f32) -> Self {
        let w0 = 2.0 * PI * freq / sample_rate;
        let alpha = w0.sin() / (2.0 * q);
        let cos_w0 = w0.cos();

        let b0 = (1.0 + cos_w0) / 2.0;
        let b1 = -(1.0 + cos_w0);
        let b2 = (1.0 + cos_w0) / 2.0;
        let a0 = 1.0 + alpha;
        let a1 = -2.0 * cos_w0;
        let a2 = 1.0 - alpha;

        Self {
            a0: b0 / a0, a1: b1 / a0, a2: b2 / a0,
            b1: a1 / a0, b2: a2 / a0,
            z1: 0.0, z2: 0.0,
        }
    }

    fn new_lowpass(sample_rate: f32, freq: f32, q: f32) -> Self {
        let w0 = 2.0 * PI * freq / sample_rate;
        let alpha = w0.sin() / (2.0 * q);
        let cos_w0 = w0.cos();

        let b0 = (1.0 - cos_w0) / 2.0;
        let b1 = 1.0 - cos_w0;
        let b2 = (1.0 - cos_w0) / 2.0;
        let a0 = 1.0 + alpha;
        let a1 = -2.0 * cos_w0;
        let a2 = 1.0 - alpha;

        Self {
            a0: b0 / a0, a1: b1 / a0, a2: b2 / a0,
            b1: a1 / a0, b2: a2 / a0,
            z1: 0.0, z2: 0.0,
        }
    }
}
