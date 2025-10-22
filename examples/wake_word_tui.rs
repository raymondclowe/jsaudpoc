//! Wake Word TUI Demo
//! Multi-pane TUI: status + live sound level + debug widgets

use std::io;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use audio_transcribe_cli::wake_word::WakeWordDetector;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossterm::event::{self, Event, KeyCode};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders, Paragraph, Gauge};
use ratatui::Terminal;

fn main() -> Result<(), io::Error> {
    // Shared state between audio callback and UI
    let current_rms = Arc::new(Mutex::new(0f32));
    let peak_rms = Arc::new(Mutex::new(0f32));
    let status_text = Arc::new(Mutex::new(String::from("Listening...")));
    let audio_buffer = Arc::new(Mutex::new(Vec::new()));

    // Wake Word Detector
    let mut detector = WakeWordDetector::new();
    // For demonstration, we'll create a dummy template.
    // In a real application, you would load a pre-trained template.
    let dummy_template_features = ndarray::Array2::zeros((50, 13));
    detector.set_template(dummy_template_features);
    detector.set_threshold(0.9); // High threshold for dummy template
    let detector = Arc::new(Mutex::new(detector));

    // Spawn audio capture stream and keep stream in scope so it isn't dropped
    let _stream = match start_audio_stream(
        Arc::clone(&current_rms),
        Arc::clone(&peak_rms),
        Arc::clone(&audio_buffer),
    ) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to start audio stream: {}", e);
            // continue without stream
            // create a dummy stream via None equivalent - we'll just continue
            // but return early would stop demo; we continue with zero levels
            // Use Option<Stream>? but to keep minimal changes, just continue
            // by not having a stream.
            // For simplicity, just panic to surface the error.
            panic!("Failed to start audio stream: {}", e);
        }
    };

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    let backend = CrosstermBackend::new(&mut stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut last_draw = Instant::now();
    let mut last_detection = Instant::now();

    loop {
        // draw UI
        terminal.draw(|f| {
            let size = f.size();
            let cols = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
                .split(size);

            // Left: status / logs
            let status_block = Block::default().title("Status").borders(Borders::ALL);
            let status = status_text.lock().unwrap().clone();
            let paragraph = Paragraph::new(status).block(status_block);
            f.render_widget(paragraph, cols[0]);

            // Right: sound level gauge
            let level_block = Block::default().title("Sound Level").borders(Borders::ALL);
            let rms = *current_rms.lock().unwrap();
            let value = rms;
            let percent = (value.clamp(0.0, 1.0) * 100.0) as u16;
            let label = format!("{:.2}", value);
            let gauge = Gauge::default()
                .block(level_block)
                .gauge_style(Style::default().fg(Color::Green))
                .percent(percent)
                .label(label);
            f.render_widget(gauge, cols[1]);
        })?;

        // Wake word detection logic
        if last_detection.elapsed() > Duration::from_millis(500) {
            let mut buffer = audio_buffer.lock().unwrap();
            if !buffer.is_empty() {
                let audio_data = buffer.clone();
                buffer.clear();

                let mut detector = detector.lock().unwrap();
                let mut status = status_text.lock().unwrap();

                match detector.detect(&audio_data) {
                    Ok((detected, similarity)) => {
                        if detected {
                            *status = format!("Wake Word DETECTED! (Similarity: {:.2})", similarity);
                        } else {
                            *status = format!("Listening... (Similarity: {:.2})", similarity);
                        }
                    }
                    Err(e) => {
                        *status = format!("Error: {}", e);
                    }
                }
            }
            last_detection = Instant::now();
        }

        // throttle draw
        if last_draw.elapsed() < Duration::from_millis(80) {
            // handle input but continue
            if event::poll(Duration::from_millis(20))? {
                if let Event::Key(key) = event::read()? {
                    if key.code == KeyCode::Char('q') {
                        break;
                    }
                    if key.code == KeyCode::Char('d') {
                        let mut s = status_text.lock().unwrap();
                        *s = "Wake word candidate detected!".to_string();
                    }
                    if key.code == KeyCode::Char('c') {
                        let mut s = status_text.lock().unwrap();
                        *s = "Wake word confirmed by Whisper!".to_string();
                    }
                }
            }
            continue;
        }

        last_draw = Instant::now();

        // handle input
        if event::poll(Duration::from_millis(10))? {
            if let Event::Key(key) = event::read()? {
                if key.code == KeyCode::Char('q') {
                    break;
                }
            }
        }
    }

    disable_raw_mode()
}

fn start_audio_stream(
    current_rms: Arc<Mutex<f32>>,
    peak_rms: Arc<Mutex<f32>>,
    audio_buffer: Arc<Mutex<Vec<f32>>>,
) -> Result<cpal::Stream, anyhow::Error> {
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or_else(|| anyhow::anyhow!("No input device available"))?;
    let config = device.default_input_config()?;
    // Create the stream according to sample format and return it; caller will keep it alive
    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => {
            build_input_stream_f32(&device, &config.into(), current_rms, peak_rms, audio_buffer)?
        }
        cpal::SampleFormat::I16 => {
            build_input_stream_i16(&device, &config.into(), current_rms, peak_rms, audio_buffer)?
        }
        cpal::SampleFormat::U16 => {
            build_input_stream_u16(&device, &config.into(), current_rms, peak_rms, audio_buffer)?
        }
        _ => build_input_stream_f32(&device, &config.into(), current_rms, peak_rms, audio_buffer)?,
    };

    stream.play()?;

    Ok(stream)
}
fn build_input_stream_f32(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    current_rms: Arc<Mutex<f32>>,
    peak_rms: Arc<Mutex<f32>>,
    audio_buffer: Arc<Mutex<Vec<f32>>>,
) -> Result<cpal::Stream, anyhow::Error> {
    let err_fn = |err| eprintln!("Audio stream error: {}", err);
    let channels = config.channels as usize;
    let stream = device.build_input_stream(
        config,
        move |data: &[f32], _| {
            // Append to buffer for wake word detection
            if let Ok(mut buffer) = audio_buffer.lock() {
                buffer.extend_from_slice(data);
                // Optional: limit buffer size to avoid memory issues
                const MAX_BUFFER_SAMPLES: usize = 16000 * 2; // 2 seconds
                if buffer.len() > MAX_BUFFER_SAMPLES {
                    buffer.drain(0..buffer.len() - MAX_BUFFER_SAMPLES);
                }
            }

            let mut sum = 0f32;
            let mut count = 0usize;
            for frame in data.chunks(channels) {
                if let Some(&s) = frame.get(0) {
                    sum += s * s;
                    count += 1;
                }
            }
            if count > 0 {
                let rms = (sum / count as f32).sqrt();
                {
                    let mut cur = current_rms.lock().unwrap();
                    *cur = rms;
                }
                {
                    let mut peak = peak_rms.lock().unwrap();
                    if rms > *peak {
                        *peak = rms;
                    } else {
                        *peak *= 0.95;
                    }
                }
            }
        },
        err_fn,
        None,
    )?;
    Ok(stream)
}

fn build_input_stream_i16(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    current_rms: Arc<Mutex<f32>>,
    peak_rms: Arc<Mutex<f32>>,
    audio_buffer: Arc<Mutex<Vec<f32>>>,
) -> Result<cpal::Stream, anyhow::Error> {
    let err_fn = |err| eprintln!("Audio stream error: {}", err);
    let channels = config.channels as usize;
    let stream = device.build_input_stream(
        config,
        move |data: &[i16], _| {
            // Convert and append to buffer
            let f32_data: Vec<f32> = data.iter().map(|&s| s as f32 / i16::MAX as f32).collect();
            if let Ok(mut buffer) = audio_buffer.lock() {
                buffer.extend_from_slice(&f32_data);
                const MAX_BUFFER_SAMPLES: usize = 16000 * 2; // 2 seconds
                if buffer.len() > MAX_BUFFER_SAMPLES {
                    buffer.drain(0..buffer.len() - MAX_BUFFER_SAMPLES);
                }
            }

            let mut sum = 0f32;
            let mut count = 0usize;
            for frame in data.chunks(channels) {
                if let Some(&s) = frame.get(0) {
                    let f = s as f32 / i16::MAX as f32;
                    sum += f * f;
                    count += 1;
                }
            }
            if count > 0 {
                let rms = (sum / count as f32).sqrt();
                {
                    let mut cur = current_rms.lock().unwrap();
                    *cur = rms;
                }
                {
                    let mut peak = peak_rms.lock().unwrap();
                    if rms > *peak {
                        *peak = rms;
                    } else {
                        *peak *= 0.95;
                    }
                }
            }
        },
        err_fn,
        None,
    )?;
    Ok(stream)
}

fn build_input_stream_u16(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    current_rms: Arc<Mutex<f32>>,
    peak_rms: Arc<Mutex<f32>>,
    audio_buffer: Arc<Mutex<Vec<f32>>>,
) -> Result<cpal::Stream, anyhow::Error> {
    let err_fn = |err| eprintln!("Audio stream error: {}", err);
    let channels = config.channels as usize;
    let stream = device.build_input_stream(
        config,
        move |data: &[u16], _| {
            // Convert and append to buffer
            let f32_data: Vec<f32> = data
                .iter()
                .map(|&s| (s as f32 / u16::MAX as f32) * 2.0 - 1.0)
                .collect();
            if let Ok(mut buffer) = audio_buffer.lock() {
                buffer.extend_from_slice(&f32_data);
                const MAX_BUFFER_SAMPLES: usize = 16000 * 2; // 2 seconds
                if buffer.len() > MAX_BUFFER_SAMPLES {
                    buffer.drain(0..buffer.len() - MAX_BUFFER_SAMPLES);
                }
            }

            let mut sum = 0f32;
            let mut count = 0usize;
            for frame in data.chunks(channels) {
                if let Some(&s) = frame.get(0) {
                    // u16 is 0..65535, convert to -1.0..1.0
                    let f = (s as f32 / u16::MAX as f32) * 2.0 - 1.0;
                    sum += f * f;
                    count += 1;
                }
            }
            if count > 0 {
                let rms = (sum / count as f32).sqrt();
                {
                    let mut cur = current_rms.lock().unwrap();
                    *cur = rms;
                }
                {
                    let mut peak = peak_rms.lock().unwrap();
                    if rms > *peak {
                        *peak = rms;
                    } else {
                        *peak *= 0.95;
                    }
                }
            }
        },
        err_fn,
        None,
    )?;
    Ok(stream)
}
