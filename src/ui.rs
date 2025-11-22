use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{Axis, Block, Borders, Chart, Dataset, Gauge, GraphType, Paragraph},
    Terminal,
};
use std::{io, sync::{Arc, Mutex}, time::{Duration, Instant}};
use spectrum_analyzer::{samples_fft_to_spectrum, FrequencyLimit};
use crate::dsp_params::SharedDspParams;

pub struct App {
    pub params: SharedDspParams,
    pub audio_buffer: Arc<Mutex<Vec<f32>>>,
    pub should_quit: bool,
    pub rms_level: f32,
}

impl App {
    pub fn new(params: SharedDspParams, audio_buffer: Arc<Mutex<Vec<f32>>>) -> Self {
        Self {
            params,
            audio_buffer,
            should_quit: false,
            rms_level: 0.0,
        }
    }

    pub fn on_tick(&mut self) {
        // Calculate RMS from the latest buffer for the gauge
        let buffer = self.audio_buffer.lock().unwrap();
        if !buffer.is_empty() {
            let sum_sq: f32 = buffer.iter().map(|&x| x * x).sum();
            self.rms_level = (sum_sq / buffer.len() as f32).sqrt();
        }
    }
}

pub fn run_tui(app: Arc<Mutex<App>>) -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let tick_rate = Duration::from_millis(50); // 20 FPS
    let mut last_tick = Instant::now();

    loop {
        let mut app_guard = app.lock().unwrap();
        
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Percentage(50), // Spectrum
                    Constraint::Percentage(20), // VU Meter
                    Constraint::Percentage(30), // Controls/Status
                ])
                .split(f.size());

            // 1. Spectrum Analyzer
            let buffer = app_guard.audio_buffer.lock().unwrap().clone();
            let mut spectrum_data = vec![];
            
            if buffer.len() >= 256 {
                // Take last 1024 samples or max available power of 2
                let fft_len = 1024.min(buffer.len().next_power_of_two() / 2); 
                // Actually spectrum_analyzer needs power of 2. 
                // Let's just take a slice if possible.
                if fft_len > 0 {
                     // Simple FFT
                    let spectrum = samples_fft_to_spectrum(
                        &buffer[0..fft_len], // Just take the beginning for now, ideally a ring buffer
                        48000,
                        FrequencyLimit::Range(0.0, 5000.0), // Focus on 0-5kHz
                        Some(&|val, _point| {
                            // Scaling function if needed
                            val
                        }),
                    );
                    
                    if let Ok(spec) = spectrum {
                        for (fr, val) in spec.data().iter() {
                            spectrum_data.push((fr.val() as f64, val.val() as f64));
                        }
                    }
                }
            }

            let datasets = vec![
                Dataset::default()
                    .name("Spectrum")
                    .marker(symbols::Marker::Braille)
                    .graph_type(GraphType::Line)
                    .style(Style::default().fg(Color::Cyan))
                    .data(&spectrum_data),
            ];

            let chart = Chart::new(datasets)
                .block(Block::default().title("Frequency Spectrum (0-5kHz)").borders(Borders::ALL))
                .x_axis(Axis::default().title("Freq (Hz)").bounds([0.0, 5000.0]))
                .y_axis(Axis::default().title("Mag").bounds([0.0, 100.0])); // Magnitude scaling might need tuning
            f.render_widget(chart, chunks[0]);

            // 2. VU Meter
            let rms = app_guard.rms_level;
            let gauge = Gauge::default()
                .block(Block::default().title("Output Level").borders(Borders::ALL))
                .gauge_style(Style::default().fg(if rms > 0.8 { Color::Red } else { Color::Green }))
                .ratio((rms * 5.0).clamp(0.0, 1.0) as f64);
            f.render_widget(gauge, chunks[1]);

            // 3. Controls & Status
            let params = app_guard.params.lock().unwrap();
            let status_text = vec![
                Line::from(vec![
                    Span::styled("Gain (v+/-): ", Style::default().fg(Color::Yellow)),
                    Span::raw(format!("{:.2}", params.gain)),
                ]),
                Line::from(vec![
                    Span::styled("Threshold (t+/-): ", Style::default().fg(Color::Yellow)),
                    Span::raw(format!("{:.4}", params.noise_threshold)),
                ]),
                Line::from(vec![
                    Span::styled("Freq Shift (f+/-): ", Style::default().fg(Color::Yellow)),
                    Span::raw(format!("{:.1} Hz", params.freq_shift)),
                ]),
                Line::from(vec![
                    Span::styled("Bandpass Filter (b): ", Style::default().fg(Color::Yellow)),
                    Span::styled(
                        if params.filter_enabled { "ON" } else { "OFF" },
                        Style::default().fg(if params.filter_enabled { Color::Green } else { Color::Red })
                    ),
                ]),
                Line::from(""),
                Line::from(Span::styled("Press 'q' to quit", Style::default().add_modifier(Modifier::BOLD))),
            ];
            
            let paragraph = Paragraph::new(status_text)
                .block(Block::default().title("Controls").borders(Borders::ALL));
            f.render_widget(paragraph, chunks[2]);
        })?;

        // Event Handling
        if event::poll(Duration::from_millis(10))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    // Clone params Arc to avoid holding app_guard lock
                    let params_arc = app_guard.params.clone();
                    
                    match key.code {
                        KeyCode::Char('q') => {
                            app_guard.should_quit = true;
                            break; // Break inner loop to exit
                        }
                        KeyCode::Char('v') => {
                            let mut p = params_arc.lock().unwrap();
                            p.gain *= 1.1;
                        }
                        KeyCode::Char('V') => {
                            let mut p = params_arc.lock().unwrap();
                            p.gain *= 0.9;
                        }
                        KeyCode::Char('t') => {
                            let mut p = params_arc.lock().unwrap();
                            p.noise_threshold *= 1.5;
                        }
                        KeyCode::Char('T') => {
                            let mut p = params_arc.lock().unwrap();
                            p.noise_threshold *= 0.6;
                        }
                        KeyCode::Char('f') => {
                            let mut p = params_arc.lock().unwrap();
                            p.freq_shift += 0.5;
                        }
                        KeyCode::Char('F') => {
                            let mut p = params_arc.lock().unwrap();
                            p.freq_shift -= 0.5;
                        }
                        KeyCode::Char('b') => {
                            let mut p = params_arc.lock().unwrap();
                            p.filter_enabled = !p.filter_enabled;
                        }
                        _ => {}
                    }
                }
            }
        }

        if last_tick.elapsed() >= tick_rate {
            app_guard.on_tick();
            last_tick = Instant::now();
        }
        
        if app_guard.should_quit {
            break;
        }
        
        // Release lock before sleeping/looping
        drop(app_guard);
        std::thread::sleep(Duration::from_millis(10));
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}
