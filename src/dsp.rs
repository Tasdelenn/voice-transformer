use anyhow::Result;
use std::f32::consts::PI;

pub struct DspProcessor {
    noise_threshold: f32,
    high_pass_state: f32,
    alpha: f32,
    // Frequency shift parameters
    phase: f32,
    freq_shift: f32,
    sample_rate: f32,
}

impl DspProcessor {
    pub fn new(sample_rate: u32, _channels: u16) -> Result<Self> {
        // Simple RC High-pass filter at ~80Hz
        let rc = 1.0 / (2.0 * PI * 80.0);
        let dt = 1.0 / sample_rate as f32;
        let alpha = rc / (rc + dt);

        Ok(Self {
            noise_threshold: 0.001,
            high_pass_state: 0.0,
            alpha,
            phase: 0.0,
            freq_shift: 5.0, // 5Hz shift
            sample_rate: sample_rate as f32,
        })
    }

    pub fn process_frame(&mut self, frame: &mut [f32]) -> Result<()> {
        for sample in frame.iter_mut() {
            let raw = *sample;
            
            // 1. Frequency Shift (Amplitude Modulation / Tremolo approach from original code)
            // This breaks the feedback loop by constantly changing the gain slightly
            self.phase += 2.0 * PI * self.freq_shift / self.sample_rate;
            if self.phase >= 2.0 * PI {
                self.phase -= 2.0 * PI;
            }
            // Original logic: *sample * (1.0 + 0.02 * phase.sin())
            let shifted = raw * (1.0 + 0.05 * self.phase.sin()); // Increased depth slightly to 0.05

            // 2. Noise Gate
            if shifted.abs() < self.noise_threshold {
                *sample = 0.0;
            } else {
                // Apply slight gain compensation
                *sample = shifted * 1.5;
            }
            
            // 3. High-pass filter (Simple DC blocker)
            // y[n] = x[n] - x[n-1] + R * y[n-1]
            // We'll stick to the simple gate + shift for now to avoid complexity
        }
        Ok(())
    }
    
    pub fn process_render_frame(&mut self, _frame: &mut [f32]) -> Result<()> {
        Ok(())
    }
}
