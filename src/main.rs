use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use dotenv::dotenv;
use hound::{WavSpec, WavWriter};
use reqwest::blocking::multipart;
use std::env;
use std::fs;
use std::sync::{Arc, Mutex};
use std::time::Duration;

// Export wake word module for examples and library usage
pub mod wake_word;

fn record_audio(duration_secs: u64) -> Result<Vec<u8>> {
    println!("Recording audio for {} seconds...", duration_secs);
    
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .context("No input device available")?;
    
    println!("Using input device: {}", device.name()?);
    
    let config = device.default_input_config()?;
    println!("Default input config: {:?}", config);
    
    let sample_rate = config.sample_rate().0;
    let channels = config.channels() as u16;
    
    let spec = WavSpec {
        channels,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    
    // Use a platform-appropriate temporary file
    #[cfg(target_os = "windows")]
    let temp_dir = "C:/tmp";
    #[cfg(not(target_os = "windows"))]
    let temp_dir = "/tmp";
    // Create the temp directory if it doesn't exist
    std::fs::create_dir_all(temp_dir)?;
    let temp_path = format!("{}/recording.wav", temp_dir);
    let writer = Arc::new(Mutex::new(WavWriter::create(&temp_path, spec)?));
    let writer_clone = Arc::clone(&writer);
    
    let err_fn = |err| eprintln!("An error occurred on stream: {}", err);
    
    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => device.build_input_stream(
            &config.into(),
            move |data: &[f32], _: &_| {
                let mut writer = writer_clone.lock().unwrap();
                for &sample in data {
                    let sample = (sample * i16::MAX as f32) as i16;
                    writer.write_sample(sample).unwrap();
                }
            },
            err_fn,
            None,
        )?,
        cpal::SampleFormat::I16 => device.build_input_stream(
            &config.into(),
            move |data: &[i16], _: &_| {
                let mut writer = writer_clone.lock().unwrap();
                for &sample in data {
                    writer.write_sample(sample).unwrap();
                }
            },
            err_fn,
            None,
        )?,
        cpal::SampleFormat::U16 => device.build_input_stream(
            &config.into(),
            move |data: &[u16], _: &_| {
                let mut writer = writer_clone.lock().unwrap();
                for &sample in data {
                    let sample = (sample as i32 - 32768) as i16;
                    writer.write_sample(sample).unwrap();
                }
            },
            err_fn,
            None,
        )?,
        _ => return Err(anyhow::anyhow!("Unsupported sample format")),
    };
    
    stream.play()?;
    
    println!("Recording...");
    std::thread::sleep(Duration::from_secs(duration_secs));
    
    drop(stream);
    println!("Recording complete!");
    
    // Finalize the writer
    let writer = Arc::try_unwrap(writer)
        .map_err(|_| anyhow::anyhow!("Failed to unwrap writer"))?
        .into_inner()
        .unwrap();
    
    writer.finalize()?;
    
    // Read the file back
        let wav_data = fs::read(&temp_path)?;
    
    // Clean up
        fs::remove_file(&temp_path).ok();
    
    Ok(wav_data)
}

fn transcribe_audio(audio_data: Vec<u8>) -> Result<String> {
    println!("Sending audio to local Whisper for transcription...");
    let client = reqwest::blocking::Client::new();
    let part = multipart::Part::bytes(audio_data)
        .file_name("audio.wav")
        .mime_str("audio/wav")?;
    let form = multipart::Form::new().part("file", part);
    let url = "http://tc3.local:8085/transcribe";
    let response = client
        .post(url)
        .multipart(form)
        .send()
        .context("Failed to send request to local Whisper API")?;
    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().unwrap_or_default();
        return Err(anyhow::anyhow!(
            "Local Whisper API error ({}): {}",
            status,
            error_text
        ));
    }
    let result: serde_json::Value = response.json()?;
    let text = result.get("text")
        .and_then(|v| v.as_str())
        .unwrap_or("(No transcription returned)")
        .to_string();
    Ok(text)
}

fn main() -> Result<()> {
    // Load .env file
    dotenv().ok();
    
    println!("Audio Transcription CLI (Local Whisper)");
    println!("======================");
    // Record 5 seconds of audio by default
    let duration = env::var("RECORD_DURATION")
        .ok()
        .and_then(|d| d.parse().ok())
        .unwrap_or(5);
    let audio_data = record_audio(duration)?;
    println!("Audio recorded: {} bytes", audio_data.len());
    let transcription = transcribe_audio(audio_data)?;
    println!("\n======================");
    println!("Transcription Result:");
    println!("======================");
    println!("{}", transcription);
    Ok(())
}
