use anyhow::Result;
use clap::Parser;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Sample, SampleFormat};
use std::sync::{Arc, Mutex};
use std::f32::consts::PI;
use std::io::{self, Write};
use std::thread;
use std::time::{Duration, Instant};
use rustfft::{FftPlanner, num_complex::Complex};
use crossterm::{
    terminal::{self, ClearType},
    cursor, execute,
    style::{Color, Print, SetForegroundColor, ResetColor},
};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// List all available audio devices
    #[arg(long)]
    list_devices: bool,

    /// Input device ID
    #[arg(long)]
    device: Option<usize>,
}

// Cubic interpolation function for smoother audio resampling
fn cubic_interpolate(p0: f32, p1: f32, p2: f32, p3: f32, t: f32) -> f32 {
    let a0 = p3 - p2 - p0 + p1;
    let a1 = p0 - p1 - a0;
    let a2 = p2 - p0;
    let a3 = p1;
    a0 * t * t * t + a1 * t * t + a2 * t + a3
}

// FFT visualization function
fn perform_fft_visualization(
    input_buffer: &[f32], 
    sample_rate: f32,
    fft_size: usize
) -> Vec<f32> {
    if input_buffer.len() < fft_size {
        return vec![0.0; fft_size / 2];
    }
    
    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(fft_size);
    
    // Prepare input data with windowing (Hanning window)
    let mut buffer: Vec<Complex<f32>> = input_buffer
        .iter()
        .take(fft_size)
        .enumerate()
        .map(|(i, &sample)| {
            let window = 0.5 * (1.0 - (2.0 * PI * i as f32 / (fft_size - 1) as f32).cos());
            Complex::new(sample * window, 0.0)
        })
        .collect();
    
    // Perform FFT
    fft.process(&mut buffer);
    
    // Calculate magnitude spectrum (only first half due to symmetry)
    let magnitude_spectrum: Vec<f32> = buffer
        .iter()
        .take(fft_size / 2)
        .map(|c| c.norm())
        .collect();
    
    magnitude_spectrum
}

// Display frequency spectrum visualization
fn display_frequency_spectrum_animated(
    spectrum: &[f32], 
    sample_rate: f32, 
    width: usize,
    height: usize,
    frame_count: u64
) {
    if spectrum.is_empty() {
        return;
    }
    
    // Clear screen and move to top
    print!("\x1B[2J\x1B[H");
    
    let max_magnitude = spectrum.iter().fold(0.0f32, |max, &val| max.max(val));
    if max_magnitude <= 0.0 {
        println!("No audio signal detected.");
        return;
    }
    
    let freq_resolution = sample_rate / spectrum.len() as f32 / 2.0;
    
    println!("\n🎵 CONTINUOUS FREQUENCY SPECTRUM 🎵 Frame: {}", frame_count);
    println!("═══════════════════════════════════════════════════════════════════════════════");
    println!("Sample Rate: {:.0} Hz | FFT Size: {} | Freq Resolution: {:.1} Hz/bin", 
             sample_rate, spectrum.len() * 2, freq_resolution);
    println!("Max Magnitude: {:.4}", max_magnitude);
    println!("═══════════════════════════════════════════════════════════════════════════════");
    
    // Display frequency bars
    for row in 0..height {
        let threshold = (height - row) as f32 / height as f32;
        print!("|");
        
        for bin in 0..width.min(spectrum.len()) {
            let normalized_magnitude = spectrum[bin] / max_magnitude;
            let freq = bin as f32 * freq_resolution;
            
            if normalized_magnitude >= threshold {
                // Color coding by frequency range
                if freq < 250.0 {
                    print!("\x1B[31m█\x1B[0m"); // Red for bass (0-250Hz)
                } else if freq < 500.0 {
                    print!("\x1B[33m█\x1B[0m"); // Yellow for low-mid (250-500Hz)
                } else if freq < 2000.0 {
                    print!("\x1B[32m█\x1B[0m"); // Green for mid (500-2000Hz)
                } else if freq < 6000.0 {
                    print!("\x1B[36m█\x1B[0m"); // Cyan for high-mid (2-6kHz)
                } else {
                    print!("\x1B[35m█\x1B[0m"); // Magenta for high (6kHz+)
                }
            } else {
                print!(" ");
            }
        }
        println!("| {:.1}%", threshold * 100.0);
    }
    
    // Frequency labels
    print!("└");
    for _ in 0..width.min(spectrum.len()) {
        print!("─");
    }
    println!("┘");
    
    // Display frequency scale
    print!(" ");
    for i in (0..width.min(spectrum.len())).step_by(width / 8) {
        let freq = i as f32 * freq_resolution;
        if freq < 1000.0 {
            print!("{:>5.0}Hz ", freq);
        } else {
            print!("{:>4.1}kHz", freq / 1000.0);
        }
        // Add spaces to align properly
        for _ in 0..(width / 8).saturating_sub(7) {
            print!(" ");
        }
    }
    println!();
    
    // Legend
    println!("\nColor Legend: \x1B[31m█\x1B[0m Bass(0-250Hz) \x1B[33m█\x1B[0m Low-Mid(250-500Hz) \x1B[32m█\x1B[0m Mid(500-2kHz) \x1B[36m█\x1B[0m High-Mid(2-6kHz) \x1B[35m█\x1B[0m High(6kHz+)");
}

fn main() -> Result<()> {
    let args = Args::parse();
    let host = cpal::default_host();

    if args.list_devices {
        println!("Input devices:");
        for (idx, device) in host.input_devices()?.enumerate() {
            println!("{}: {}", idx, device.name()?);
        }
        return Ok(());
    }

    // Get input device
    let input_device = if let Some(device_idx) = args.device {
        let devices: Vec<_> = host.input_devices()?.collect();
        devices
            .get(device_idx)
            .ok_or_else(|| anyhow::anyhow!("Invalid device ID"))?
            .clone()
    } else {
        host.default_input_device()
            .ok_or_else(|| anyhow::anyhow!("No default input device"))?
    };

    println!("Using input device: {}", input_device.name()?);

    // Debug: Print supported input configurations
    println!("\nSupported input configurations:");
    for (i, config) in input_device.supported_input_configs()?.enumerate() {
        println!("  {}: {:?}", i, config);
    }

    // Get output device
    let output_device = host
        .default_output_device()
        .ok_or_else(|| anyhow::anyhow!("No default output device"))?;

    println!("\nUsing output device: {}", output_device.name()?);

    // Debug: Print supported output configurations
    println!("\nSupported output configurations:");
    for (i, config) in output_device.supported_output_configs()?.enumerate() {
        println!("  {}: {:?}", i, config);
    }

    // Create a buffer to store audio samples
    let audio_buffer = Arc::new(Mutex::new(Vec::<f32>::new()));
    
    // FFT visualization buffers
    let fft_size = 1024usize;
    let fft_input_buffer = Arc::new(Mutex::new(Vec::<f32>::new()));
    let fft_output_buffer = Arc::new(Mutex::new(Vec::<f32>::new()));
    let visualization_enabled = Arc::new(Mutex::new(false));
    let last_visualization_time = Arc::new(Mutex::new(Instant::now()));
    let last_frame_data = Arc::new(Mutex::new((Vec::<f32>::new(), Vec::<f32>::new()))); // (input, output)
    
    // Audio processing parameters (adjustable)
    let sample_rate = 44100.0;
    let volume = Arc::new(Mutex::new(0.9f32));
    let noise_threshold = Arc::new(Mutex::new(0.01f32));
    let attack_time = Arc::new(Mutex::new(0.01f32));  // 10ms attack
    let release_time = Arc::new(Mutex::new(0.1f32));  // 100ms release
    let smoothing = Arc::new(Mutex::new(0.7f32));     // Smoothing factor (0-1)
    let freq_shift = Arc::new(Mutex::new(5.0f32));
    let buffer_size_limit = Arc::new(Mutex::new(2400usize)); // Smaller buffer for less latency
    let phase_accumulator = Arc::new(Mutex::new(0.0f32));
    
    // Noise gate state
    let envelope = Arc::new(Mutex::new(0.0f32));
    let adaptive_threshold = Arc::new(Mutex::new(0.01f32));
    
    // Simple resampling ratio
    let resample_ratio = 48000.0 / 44100.0;

// Configure the audio stream - use separate configs for input and output
    let mut input_configs = input_device.supported_input_configs()?;
    let input_config = input_configs.next()
        .ok_or(anyhow::anyhow!("No supported input configuration found"))?
        .with_max_sample_rate();
    let input_stream_config: cpal::StreamConfig = input_config.config();
    
    let mut output_configs = output_device.supported_output_configs()?;
    let output_config = output_configs.next()
        .ok_or(anyhow::anyhow!("No supported output configuration found"))?
        .with_max_sample_rate();
    let output_stream_config: cpal::StreamConfig = output_config.config();
    
    println!("\nUsing input config: {:?}", input_stream_config);
    println!("Using output config: {:?}", output_stream_config);

    // Build the input stream
    let input_data = audio_buffer.clone();
    let fft_input_clone = fft_input_buffer.clone();
    let phase_acc = phase_accumulator.clone();
    let vol_clone = volume.clone();
    let noise_clone = noise_threshold.clone();
    let attack_clone = attack_time.clone();
    let release_clone = release_time.clone();
    let smoothing_clone = smoothing.clone();
    let envelope_clone = envelope.clone();
    let adaptive_threshold_clone = adaptive_threshold.clone();
    let freq_clone = freq_shift.clone();
    let buffer_limit_clone = buffer_size_limit.clone();
    
    let input_stream = input_device.build_input_stream(
        &input_stream_config,
        move |data: &[f32], _: &cpal::InputCallbackInfo| {
            let mut buffer = input_data.lock().unwrap();
            let mut phase = phase_acc.lock().unwrap();
            
            // Get current parameters
            let volume = vol_clone.lock().unwrap();
            let noise_threshold = noise_clone.lock().unwrap();
            let attack = attack_clone.lock().unwrap();
            let release = release_clone.lock().unwrap();
            let smoothing_factor = smoothing_clone.lock().unwrap();
            let mut envelope = envelope_clone.lock().unwrap();
            let mut adaptive_thresh = adaptive_threshold_clone.lock().unwrap();
            let freq_shift = freq_clone.lock().unwrap();
            let buffer_size_limit = buffer_limit_clone.lock().unwrap();
            
            // Update adaptive threshold based on recent signal levels
            let signal_level = data.iter().map(|x| x.abs()).sum::<f32>() / data.len() as f32;
            *adaptive_thresh = *adaptive_thresh * 0.95 + signal_level * 0.05;
            
            // Process each sample with smoother algorithms
            for sample in data {
                // Improved frequency shifting with smoother modulation
                *phase += 2.0 * PI * *freq_shift / sample_rate;
                if *phase >= 2.0 * PI {
                    *phase -= 2.0 * PI;
                }
                // Use a blend of sine and cosine for smoother modulation
                let mod_amount = 0.015; // Reduced modulation depth
                let shifted_sample = *sample * (1.0 + mod_amount * (0.7 * phase.sin() + 0.3 * phase.cos()));
                
                // Advanced noise gate with attack/release and smoothing
                let sample_level = shifted_sample.abs();
                let target_envelope = if sample_level > *noise_threshold {
                    sample_level
                } else {
                    0.0
                };
                
                // Apply attack/release timing
                let time_constant = if target_envelope > *envelope {
                    *attack  // Attack time when signal is rising
                } else {
                    *release // Release time when signal is falling
                };
                
                // Update envelope with smoothing
                let alpha = (-1.0 / (sample_rate * time_constant)).exp();
                *envelope = *envelope * alpha + target_envelope * (1.0 - alpha);
                *envelope = *envelope * *smoothing_factor + target_envelope * (1.0 - *smoothing_factor);
                
                // Enhanced noise gate with smoother transition curve
                let gate_multiplier = if *envelope > *noise_threshold {
                    1.0
                } else {
                    let ratio = *envelope / *noise_threshold;
                    let curve = ratio.powf(1.5); // Gentler power curve
                    curve * (0.15 + 0.85 * ratio) // Smoother transition near threshold
                };
                
                let processed_sample = shifted_sample * *volume * gate_multiplier;
                
                buffer.push(processed_sample);
                
                // Collect data for FFT visualization
                if let Ok(mut fft_buffer) = fft_input_clone.try_lock() {
                    fft_buffer.push(processed_sample);
                    // Keep FFT buffer at fixed size for consistent visualization
                    if fft_buffer.len() > fft_size {
                        let excess = fft_buffer.len() - fft_size;
                        fft_buffer.drain(0..excess);
                    }
                }
                
                // Enhanced sample interpolation
                if buffer.len() % 441 == 0 && buffer.len() > 2 {
                    // Cubic interpolation using 4 points
                    let p0 = buffer[buffer.len() - 2];
                    let p1 = buffer[buffer.len() - 1];
                    let p2 = processed_sample;
                    let p3 = processed_sample; // Future sample (approximated)
                    let t = 0.5; // Interpolation point
                    let interpolated = cubic_interpolate(p0, p1, p2, p3, t);
                    buffer.push(interpolated);
                }
            }
            // Keep buffer size manageable
            if buffer.len() > *buffer_size_limit {
                let excess = buffer.len() - *buffer_size_limit;
                buffer.drain(0..excess);
            }
        },
        move |err| eprintln!("An error occurred on the input stream: {}", err),
        None,
    )?;

    // Build the output stream
    let output_data = audio_buffer.clone();
    let output_stream = output_device.build_output_stream(
        &output_stream_config,
        move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
            let mut buffer = output_data.lock().unwrap();
            if buffer.len() >= data.len() {
                // Copy data and remove from buffer
                for (i, output_sample) in data.iter_mut().enumerate() {
                    *output_sample = buffer[i] * 0.9; // Higher volume but still prevent feedback
                }
                buffer.drain(0..data.len());
            } else {
                // Fill with silence if not enough data
                for output_sample in data.iter_mut() {
                    *output_sample = 0.0;
                }
            }
        },
        move |err| eprintln!("An error occurred on the output stream: {}", err),
        None,
    )?;

    // Start the streams
    input_stream.play()?;
    output_stream.play()?;

    // User interface for real-time adjustments in main thread
    let vol = volume.clone();
    let noise = noise_threshold.clone();
    let attack = attack_time.clone();
    let release = release_time.clone();
    let smooth = smoothing.clone();
    let freq = freq_shift.clone();
    let buffer_limit = buffer_size_limit.clone();

    println!("\nVoice transformer started! Audio is processing...");
    
    // Function to create progress bar
    let create_bar = |value: f32, min: f32, max: f32, width: usize| -> String {
        let normalized = ((value - min) / (max - min)).clamp(0.0, 1.0);
        let filled = (normalized * width as f32) as usize;
        let empty = width - filled;
        format!("[{}{}]", "█".repeat(filled), "░".repeat(empty))
    };
    
    let create_bar_usize = |value: usize, min: usize, max: usize, width: usize| -> String {
        let normalized = ((value - min) as f32 / (max - min) as f32).clamp(0.0, 1.0);
        let filled = (normalized * width as f32) as usize;
        let empty = width - filled;
        format!("[{}{}]", "█".repeat(filled), "░".repeat(empty))
    };
    
    // Function to display current settings with progress bars
    let display_settings = || {
        let vol_val = *vol.lock().unwrap();
        let noise_val = *noise.lock().unwrap();
        let attack_val = *attack.lock().unwrap();
        let release_val = *release.lock().unwrap();
        let smooth_val = *smooth.lock().unwrap();
        let freq_val = *freq.lock().unwrap();
        let buf_val = *buffer_limit.lock().unwrap();
        
        println!("\n================== Current Settings ==================");
        println!("Volume (0.0 - 1.0)......: {:.2}..{}", vol_val, create_bar(vol_val, 0.0, 1.0, 20));
        println!("Noise Gate (0.0 - 0.1)..: {:.3}.{}", noise_val, create_bar(noise_val, 0.0, 0.1, 20));
        println!("Attack Time (0.0 - 0.1).: {:.3}.{}", attack_val, create_bar(attack_val, 0.0, 0.1, 20));
        println!("Release Time (0.0 - 0.5): {:.3}.{}", release_val, create_bar(release_val, 0.0, 0.5, 20));
        println!("Smoothing (0.0 - 1.0)...: {:.2}..{}", smooth_val, create_bar(smooth_val, 0.0, 1.0, 20));
        println!("Freq Shift (0 - 20 Hz)..: {:.1}...{}", freq_val, create_bar(freq_val, 0.0, 20.0, 20));
        println!("Buffer (0 - 10000)......: {}..{}", buf_val, create_bar_usize(buf_val, 0, 10000, 20));
        println!("======================================================");
    };
    
    // Display initial settings
    display_settings();
    
    let vis_enabled = visualization_enabled.clone();
    let fft_input_vis = fft_input_buffer.clone();
    let fft_output_vis = fft_output_buffer.clone();
    let last_vis_time = last_visualization_time.clone();
    
    loop {
        print!("\nCommands: (v)olume, (n)oise, (a)ttack, (r)elease, (s)moothing, (f)req shift, (b)uffer, (w)aveform viz, (d)efault, (i)nfo, (q)uit: ");
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();
        
        match input.chars().next() {
            Some('v') => {
                print!("Enter volume (0.0 to 1.0): ");
                io::stdout().flush()?;
                let mut vol_input = String::new();
                io::stdin().read_line(&mut vol_input)?;
                let new_vol: f32 = vol_input.trim().parse().unwrap_or(0.9);
                *vol.lock().unwrap() = new_vol;
                println!("Volume set to: {}", new_vol);
            },
            Some('n') => {
                print!("Enter noise threshold (0.0 to 0.1): ");
                io::stdout().flush()?;
                let mut noise_input = String::new();
                io::stdin().read_line(&mut noise_input)?;
                let new_noise: f32 = noise_input.trim().parse().unwrap_or(0.01);
                *noise.lock().unwrap() = new_noise;
                println!("Noise threshold set to: {}", new_noise);
            },
            Some('f') => {
                print!("Enter frequency shift (Hz): ");
                io::stdout().flush()?;
                let mut freq_input = String::new();
                io::stdin().read_line(&mut freq_input)?;
                let new_freq: f32 = freq_input.trim().parse().unwrap_or(5.0);
                *freq.lock().unwrap() = new_freq;
                println!("Frequency shift set to: {} Hz", new_freq);
            },
            Some('b') => {
                print!("Enter buffer size: ");
                io::stdout().flush()?;
                let mut buf_input = String::new();
                io::stdin().read_line(&mut buf_input)?;
                let new_buf: usize = buf_input.trim().parse().unwrap_or(2400);
                *buffer_limit.lock().unwrap() = new_buf;
                println!("Buffer size set to: {}", new_buf);
            },
            Some('d') => {
                // Load default settings
                *vol.lock().unwrap() = 0.8;
                *noise.lock().unwrap() = 0.01;
                *attack.lock().unwrap() = 0.01;
                *release.lock().unwrap() = 0.1;
                *smooth.lock().unwrap() = 0.7;
                *freq.lock().unwrap() = 5.0;
                *buffer_limit.lock().unwrap() = 2400;
                println!("\nDefault settings loaded!");
                display_settings();
            },
            Some('a') => {
                print!("Enter attack time (0.0 to 0.1 seconds): ");
                io::stdout().flush()?;
                let mut attack_input = String::new();
                io::stdin().read_line(&mut attack_input)?;
                let new_attack: f32 = attack_input.trim().parse().unwrap_or(0.01);
                *attack.lock().unwrap() = new_attack;
                println!("Attack time set to: {} seconds", new_attack);
            },
            Some('r') => {
                print!("Enter release time (0.0 to 0.5 seconds): ");
                io::stdout().flush()?;
                let mut release_input = String::new();
                io::stdin().read_line(&mut release_input)?;
                let new_release: f32 = release_input.trim().parse().unwrap_or(0.1);
                *release.lock().unwrap() = new_release;
                println!("Release time set to: {} seconds", new_release);
            },
            Some('s') => {
                print!("Enter smoothing factor (0.0 to 1.0): ");
                io::stdout().flush()?;
                let mut smooth_input = String::new();
                io::stdin().read_line(&mut smooth_input)?;
                let new_smooth: f32 = smooth_input.trim().parse().unwrap_or(0.7);
                *smooth.lock().unwrap() = new_smooth;
                println!("Smoothing factor set to: {}", new_smooth);
            },
            Some('w') => {
                // Start continuous frequency spectrum visualization in separate thread
                println!("\n🎵 Starting Continuous Frequency Spectrum Visualization...");
                println!("Press Enter to stop visualization and return to menu.");
                
                // Create a flag to control visualization thread
                let viz_running = Arc::new(std::sync::atomic::AtomicBool::new(true));
                let viz_running_clone = viz_running.clone();
                let fft_input_thread = fft_input_vis.clone();
                
                // Spawn visualization thread
                let viz_thread = std::thread::spawn(move || {
                    let mut frame_count = 0u64;
                    let start_time = std::time::Instant::now();
                    
                    while viz_running_clone.load(std::sync::atomic::Ordering::Relaxed) {
                        // Get current FFT buffer data
                        let buffer_data = if let Ok(buffer) = fft_input_thread.try_lock() {
                            if buffer.len() >= fft_size {
                                buffer.clone()
                            } else {
                                vec![0.0; fft_size]
                            }
                        } else {
                            vec![0.0; fft_size]
                        };
                        
                        // Perform FFT and display spectrum
                        let spectrum = perform_fft_visualization(&buffer_data, sample_rate, fft_size);
                        display_frequency_spectrum_animated(&spectrum, sample_rate, 80, 20, frame_count);
                        
                        frame_count += 1;
                        
                        // Control frame rate (adjustable: 20-50 FPS range)
                        // 20 FPS = 50ms, 30 FPS = 33ms, 50 FPS = 20ms
                        let fps = 30; // You can change this to 20-50
                        let frame_time_ms = 1000 / fps;
                        std::thread::sleep(Duration::from_millis(frame_time_ms));
                    }
                });
                
                // Wait for user input to stop visualization
                let mut input = String::new();
                io::stdin().read_line(&mut input).unwrap();
                
                // Stop visualization thread
                viz_running.store(false, std::sync::atomic::Ordering::Relaxed);
                viz_thread.join().unwrap();
                
                // Clear screen and return to main menu
                print!("\x1B[2J\x1B[H");
                println!("Exited visualization mode.");
                display_settings();
            },
            Some('i') => {
                // Show current status
                display_settings();
            },
            Some('q') => break,
            _ => println!("Invalid option. Use v, n, a, r, s, f, b, w, d, i, or q."),
        }
    }

    println!("\nStopping voice transformer...");
    Ok(())
}
