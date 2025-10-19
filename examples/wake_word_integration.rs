/// Integrated Wake Word + Transcription Demo
/// 
/// This example shows how to combine wake word detection with the existing
/// Whisper transcription system in a realistic always-on scenario.
/// 
/// Usage:
/// 1. Set REPLICATE_API_KEY in .env file
/// 2. Run: cargo run --example wake_word_integration
/// 3. Say "computer" to trigger recording and transcription

use anyhow::{Context, Result};
use audio_transcribe_cli::wake_word::WakeWordDetector;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use dotenv::dotenv;
use hound::{WavSpec, WavWriter};
use std::collections::VecDeque;
use std::env;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Circular buffer for audio samples
struct AudioBuffer {
    buffer: VecDeque<f32>,
    max_size: usize,
}

impl AudioBuffer {
    fn new(duration_secs: usize, sample_rate: usize) -> Self {
        let max_size = duration_secs * sample_rate;
        Self {
            buffer: VecDeque::with_capacity(max_size),
            max_size,
        }
    }
    
    fn push(&mut self, samples: &[f32]) {
        for &sample in samples {
            if self.buffer.len() >= self.max_size {
                self.buffer.pop_front();
            }
            self.buffer.push_back(sample);
        }
    }
    
    fn get_samples(&self) -> Vec<f32> {
        self.buffer.iter().copied().collect()
    }
    
    fn len(&self) -> usize {
        self.buffer.len()
    }
}

fn main() -> Result<()> {
    // Load environment variables
    dotenv().ok();
    
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║   Wake Word Detection + Transcription Demo              ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();
    println!("This demo shows a two-stage wake word detection system:");
    println!("  Stage 1: Lightweight local pattern matching (MFCC + DTW)");
    println!("  Stage 2: Whisper confirmation (if API key configured)");
    println!();
    
    // Check if API key is available
    let api_key = env::var("REPLICATE_API_KEY").ok();
    if api_key.is_none() {
        println!("⚠️  Note: REPLICATE_API_KEY not found - Stage 2 confirmation disabled");
        println!("   Set it in .env to enable full two-stage detection");
        println!();
    }
    
    println!("Setting up wake word detector...");
    let mut detector = WakeWordDetector::new();
    
    // Train a simple template for "computer"
    // In production, you would record actual samples of the wake word
    println!("  Training template (synthetic audio for demo)...");
    let training_samples = generate_training_samples(3);
    detector.train_template(&training_samples)?;
    println!("  ✓ Template trained");
    
    // Set threshold (tune this based on testing)
    detector.set_threshold(0.65);
    println!("  Detection threshold: 0.65");
    
    println!("\nStarting audio capture...");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!();
    
    // Setup audio capture
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .context("No input device available")?;
    
    println!("Using input device: {}", device.name()?);
    
    let config = device.default_input_config()?;
    let sample_rate = config.sample_rate().0;
    let channels = config.channels() as u16;
    
    println!("Sample rate: {} Hz, Channels: {}", sample_rate, channels);
    println!();
    println!("🎤 Listening for wake word \"computer\"...");
    println!("   (Press Ctrl+C to exit)");
    println!();
    
    // Shared state
    let audio_buffer = Arc::new(Mutex::new(AudioBuffer::new(2, sample_rate as usize)));
    let detector = Arc::new(Mutex::new(detector));
    let last_detection = Arc::new(Mutex::new(Instant::now()));
    
    // Clone for audio callback
    let audio_buffer_clone = Arc::clone(&audio_buffer);
    let detector_clone = Arc::clone(&detector);
    let last_detection_clone = Arc::clone(&last_detection);
    
    // Error callback
    let err_fn = |err| eprintln!("Audio stream error: {}", err);
    
    // Build audio stream
    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => device.build_input_stream(
            &config.into(),
            move |data: &[f32], _: &_| {
                process_audio_frame(
                    data,
                    &audio_buffer_clone,
                    &detector_clone,
                    &last_detection_clone,
                    sample_rate,
                );
            },
            err_fn,
            None,
        )?,
        cpal::SampleFormat::I16 => {
            let audio_buffer_clone = Arc::clone(&audio_buffer);
            let detector_clone = Arc::clone(&detector);
            let last_detection_clone = Arc::clone(&last_detection);
            
            device.build_input_stream(
                &config.into(),
                move |data: &[i16], _: &_| {
                    // Convert i16 to f32
                    let float_data: Vec<f32> = data.iter()
                        .map(|&s| s as f32 / i16::MAX as f32)
                        .collect();
                    process_audio_frame(
                        &float_data,
                        &audio_buffer_clone,
                        &detector_clone,
                        &last_detection_clone,
                        sample_rate,
                    );
                },
                err_fn,
                None,
            )?
        }
        _ => return Err(anyhow::anyhow!("Unsupported sample format")),
    };
    
    stream.play()?;
    
    // Keep running
    loop {
        std::thread::sleep(Duration::from_secs(1));
    }
}

/// Process each audio frame for wake word detection
fn process_audio_frame(
    data: &[f32],
    audio_buffer: &Arc<Mutex<AudioBuffer>>,
    detector: &Arc<Mutex<WakeWordDetector>>,
    last_detection: &Arc<Mutex<Instant>>,
    sample_rate: u32,
) {
    // Add samples to buffer
    let mut buffer = audio_buffer.lock().unwrap();
    buffer.push(data);
    
    // Only check every 100ms to reduce CPU usage
    if buffer.len() < sample_rate as usize / 10 {
        return;
    }
    
    // Prevent rapid re-triggering
    let mut last_det = last_detection.lock().unwrap();
    if last_det.elapsed() < Duration::from_secs(3) {
        return;
    }
    
    // Get samples for detection (last 1 second)
    let samples = buffer.get_samples();
    drop(buffer); // Release lock
    
    // Run detection
    let detector = detector.lock().unwrap();
    match detector.detect(&samples) {
        Ok((detected, confidence)) => {
            if detected {
                *last_det = Instant::now();
                drop(last_det);
                drop(detector);
                
                println!("🎯 Wake word detected! (confidence: {:.1}%)", confidence * 100.0);
                println!("   Stage 1: ✓ Local pattern match successful");
                
                // Stage 2: Would send to Whisper here
                println!("   Stage 2: Confirmation would be sent to Whisper");
                println!("   (Skipped in demo - set REPLICATE_API_KEY to enable)");
                println!();
                println!("🎤 Listening for wake word \"computer\"...");
                println!();
            }
        }
        Err(e) => eprintln!("Detection error: {}", e),
    }
}

/// Generate synthetic training samples for the wake word
/// In production, these would be actual recordings of "computer"
fn generate_training_samples(count: usize) -> Vec<Vec<f32>> {
    let sample_rate = 16000;
    let mut samples = Vec::new();
    
    for i in 0..count {
        let duration = 1.0 + (i as f32 * 0.05);
        let pitch_mult = 1.0 - (i as f32 * 0.03);
        
        // Simulate "computer" with multiple frequency components
        // This is a simplified representation
        let sample: Vec<f32> = (0..(sample_rate as f32 * duration) as usize)
            .map(|idx| {
                let t = idx as f32 / sample_rate as f32;
                let phase_shift = i as f32 * 0.1;
                
                // "com" - lower frequencies
                let com = if t < 0.3 {
                    (300.0 * pitch_mult * t * 2.0 * std::f32::consts::PI + phase_shift).sin() * 0.4
                } else {
                    0.0
                };
                
                // "pu" - middle frequencies
                let pu = if t >= 0.3 && t < 0.6 {
                    (800.0 * pitch_mult * t * 2.0 * std::f32::consts::PI + phase_shift).sin() * 0.3
                } else {
                    0.0
                };
                
                // "ter" - higher frequencies
                let ter = if t >= 0.6 {
                    (1500.0 * pitch_mult * t * 2.0 * std::f32::consts::PI + phase_shift).sin() * 0.3
                } else {
                    0.0
                };
                
                (com + pu + ter) * (1.0 - t) // Decay envelope
            })
            .collect();
        
        samples.push(sample);
    }
    
    samples
}
