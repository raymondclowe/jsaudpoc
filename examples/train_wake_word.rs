/// Wake Word Template Training Tool
/// 
/// This tool helps you create a custom wake word template by recording
/// multiple samples of your wake word and averaging them.
/// 
/// Usage:
///   cargo run --example train_wake_word
/// 
/// The tool will:
/// 1. Prompt you to say the wake word multiple times
/// 2. Record each sample
/// 3. Extract MFCC features
/// 4. Create an averaged template
/// 5. Save the template to a file

use anyhow::{Context, Result};
use audio_transcribe_cli::wake_word::WakeWordDetector;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use hound::{WavSpec, WavWriter};
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn main() -> Result<()> {
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║      Wake Word Template Training Tool                   ║");
    println!("╚══════════════════════════════════════════════════════════╝");
    println!();
    println!("This tool will help you create a custom wake word template.");
    println!("You'll record your wake word multiple times, and the tool will");
    println!("create an averaged template for detection.");
    println!();
    
    // Get wake word from user
    print!("Enter your wake word (e.g., 'computer'): ");
    io::stdout().flush()?;
    let mut wake_word = String::new();
    io::stdin().read_line(&mut wake_word)?;
    let wake_word = wake_word.trim();
    
    if wake_word.is_empty() {
        println!("Wake word cannot be empty!");
        return Ok(());
    }
    
    println!("\nWake word: \"{}\"", wake_word);
    println!();
    
    // Determine number of samples
    print!("How many samples to record? (recommended: 5-10): ");
    io::stdout().flush()?;
    let mut num_samples_str = String::new();
    io::stdin().read_line(&mut num_samples_str)?;
    let num_samples: usize = num_samples_str.trim().parse().unwrap_or(5);
    
    println!("\nWill record {} samples", num_samples);
    println!();
    
    // Setup audio device
    println!("Setting up audio device...");
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .context("No input device available")?;
    
    println!("Using device: {}", device.name()?);
    let config = device.default_input_config()?;
    let sample_rate = config.sample_rate().0;
    let channels = config.channels() as u16;
    println!("Sample rate: {} Hz, Channels: {}", sample_rate, channels);
    println!();
    
    // Record samples
    let mut samples = Vec::new();
    
    for i in 0..num_samples {
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("Sample {}/{}", i + 1, num_samples);
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        
        print!("Press Enter when ready to record...");
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        
        println!("🔴 Recording for 2 seconds...");
        println!("   Say: \"{}\"", wake_word);
        
        let audio_data = record_audio(&device, &config, 2)?;
        
        println!("✓ Sample recorded ({} samples)", audio_data.len());
        
        // Optional: save to WAV file for review
        let filename = format!("wake_word_sample_{}.wav", i + 1);
        save_wav(&filename, &audio_data, sample_rate, channels)?;
        println!("  Saved to: {}", filename);
        
        samples.push(audio_data);
        println!();
    }
    
    // Train the detector
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Training template...");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    
    let mut detector = WakeWordDetector::new();
    detector.train_template(&samples)?;
    
    println!("✓ Template trained successfully!");
    println!();
    
    // Test the template on each sample
    println!("Testing template on recorded samples:");
    for (i, sample) in samples.iter().enumerate() {
        let (detected, confidence) = detector.detect(sample)?;
        println!("  Sample {}: {} (confidence: {:.1}%)",
                 i + 1,
                 if detected { "✓" } else { "✗" },
                 confidence * 100.0);
    }
    println!();
    
    // Note about saving (in a real implementation, you'd serialize the template)
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Next Steps:");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!();
    println!("1. Your template has been created in memory");
    println!("2. Sample WAV files have been saved for review");
    println!("3. To use in production:");
    println!("   - Serialize the template to a file (e.g., with serde)");
    println!("   - Load it in your application");
    println!("   - Use detector.set_template() to activate");
    println!();
    println!("Tip: Adjust the threshold with detector.set_threshold()");
    println!("     - Lower (0.5-0.6): More sensitive, more false positives");
    println!("     - Higher (0.7-0.8): Less sensitive, fewer false positives");
    println!();
    
    Ok(())
}

/// Record audio for a specified duration
fn record_audio(
    device: &cpal::Device,
    config: &cpal::SupportedStreamConfig,
    duration_secs: u64,
) -> Result<Vec<f32>> {
    let sample_rate = config.sample_rate().0;
    let channels = config.channels();
    
    let audio_data = Arc::new(Mutex::new(Vec::new()));
    let audio_data_clone = Arc::clone(&audio_data);
    
    let err_fn = |err| eprintln!("Audio error: {}", err);
    
    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => device.build_input_stream(
            &config.clone().into(),
            move |data: &[f32], _: &_| {
                // If stereo, average channels to mono
                let mut audio = audio_data_clone.lock().unwrap();
                if channels == 1 {
                    audio.extend_from_slice(data);
                } else {
                    for chunk in data.chunks(channels as usize) {
                        let avg: f32 = chunk.iter().sum::<f32>() / channels as f32;
                        audio.push(avg);
                    }
                }
            },
            err_fn,
            None,
        )?,
        cpal::SampleFormat::I16 => device.build_input_stream(
            &config.clone().into(),
            move |data: &[i16], _: &_| {
                let mut audio = audio_data_clone.lock().unwrap();
                if channels == 1 {
                    audio.extend(data.iter().map(|&s| s as f32 / i16::MAX as f32));
                } else {
                    for chunk in data.chunks(channels as usize) {
                        let avg: f32 = chunk.iter().map(|&s| s as f32 / i16::MAX as f32).sum::<f32>() 
                                       / channels as f32;
                        audio.push(avg);
                    }
                }
            },
            err_fn,
            None,
        )?,
        _ => return Err(anyhow::anyhow!("Unsupported sample format")),
    };
    
    stream.play()?;
    std::thread::sleep(Duration::from_secs(duration_secs));
    drop(stream);
    
    let audio = Arc::try_unwrap(audio_data)
        .map_err(|_| anyhow::anyhow!("Failed to unwrap audio data"))?
        .into_inner()
        .unwrap();
    
    Ok(audio)
}

/// Save audio samples to a WAV file
fn save_wav(filename: &str, data: &[f32], sample_rate: u32, channels: u16) -> Result<()> {
    let spec = WavSpec {
        channels: 1, // We save as mono
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    
    let mut writer = WavWriter::create(filename, spec)?;
    
    for &sample in data {
        let sample_i16 = (sample * i16::MAX as f32) as i16;
        writer.write_sample(sample_i16)?;
    }
    
    writer.finalize()?;
    Ok(())
}
