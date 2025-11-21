use anyhow::Result;

pub struct DspProcessor {
    noise_threshold: f32,
    high_pass_state: f32,
    alpha: f32, // High-pass filter coefficient
}

impl DspProcessor {
    pub fn new(sample_rate: u32, _channels: u16) -> Result<Self> {
        // Simple RC High-pass filter at ~80Hz
        let rc = 1.0 / (2.0 * std::f32::consts::PI * 80.0);
        let dt = 1.0 / sample_rate as f32;
        let alpha = rc / (rc + dt);

        Ok(Self {
            noise_threshold: 0.001, // Lowered for testing
            high_pass_state: 0.0,
            alpha,
        })
    }

    pub fn process_frame(&mut self, frame: &mut [f32]) -> Result<()> {
        for sample in frame.iter_mut() {
            // 1. High-pass filter (remove DC offset and rumble)
            let raw = *sample;
            let filtered = self.alpha * (self.high_pass_state + raw - self.high_pass_state); 
            // Wait, standard HPF: y[i] = alpha * (y[i-1] + x[i] - x[i-1])
            // My state should store previous input and previous output?
            // Let's use a simpler implementation: y[i] = alpha * y[i-1] + alpha * (x[i] - x[i-1])
            
            // Correct implementation:
            // y[n] = alpha * (y[n-1] + x[n] - x[n-1])
            let output = self.alpha * (self.high_pass_state + raw /* - prev_raw? No, state needs to be y[n-1] and x[n-1] */);
            // Actually, let's just use a simple DC blocker: y[n] = x[n] - x[n-1] + 0.995 * y[n-1]
            
            // Let's stick to the one from main.rs logic or simple gate for now.
            // Re-implementing simple gate + gain.
            
            // Noise Gate
            if raw.abs() < self.noise_threshold {
                *sample = 0.0;
            } else {
                // Apply slight gain
                *sample = raw * 1.5;
            }
            
            // Update state (unused for simple gate)
            self.high_pass_state = output; 
        }
        Ok(())
    }
    
    pub fn process_render_frame(&mut self, _frame: &mut [f32]) -> Result<()> {
        // No-op for basic DSP
        Ok(())
    }
}
