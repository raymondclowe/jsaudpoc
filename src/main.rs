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
    
    // Use a temporary file
    let temp_path = "/tmp/recording.wav";
    let writer = Arc::new(Mutex::new(WavWriter::create(temp_path, spec)?));
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
    let wav_data = fs::read(temp_path)?;
    
    // Clean up
    fs::remove_file(temp_path).ok();
    
    Ok(wav_data)
}

fn transcribe_audio(api_key: &str, audio_data: Vec<u8>) -> Result<String> {
    println!("Sending audio to Replicate for transcription...");
    
    let client = reqwest::blocking::Client::new();
    
    let part = multipart::Part::bytes(audio_data)
        .file_name("audio.wav")
        .mime_str("audio/wav")?;
    
    let form = multipart::Form::new().part("file", part);
    
    let whisper_version = "vaibhavs10/incredibly-fast-whisper:3ab86df6c8f54c11309d4d1f930ac292bad43ace52d10c80d87eb258b3c9f79c";
    let url = format!(
        "https://api.replicate.com/v1/models/{}/predictions",
        whisper_version
    );
    
    // Use the replicate API to create prediction with multipart file
    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .multipart(form)
        .send()
        .context("Failed to send request to Replicate")?;
    
    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().unwrap_or_default();
        return Err(anyhow::anyhow!(
            "Replicate API error ({}): {}",
            status,
            error_text
        ));
    }
    
    let result: serde_json::Value = response.json()?;
    
    // Extract text from various possible response formats
    let text = if let Some(text) = result.get("text").and_then(|v| v.as_str()) {
        text.to_string()
    } else if let Some(output) = result.get("output") {
        if let Some(text) = output.get("text").and_then(|v| v.as_str()) {
            text.to_string()
        } else if let Some(text_str) = output.as_str() {
            text_str.to_string()
        } else {
            serde_json::to_string_pretty(&output)?
        }
    } else {
        "(No transcription returned)".to_string()
    };
    
    Ok(text)
}

fn main() -> Result<()> {
    // Load .env file
    dotenv().ok();
    
    let api_key = env::var("REPLICATE_API_KEY")
        .context("REPLICATE_API_KEY not found in environment or .env file")?;
    
    println!("Audio Transcription CLI");
    println!("======================");
    
    // Record 5 seconds of audio by default
    let duration = env::var("RECORD_DURATION")
        .ok()
        .and_then(|d| d.parse().ok())
        .unwrap_or(5);
    
    let audio_data = record_audio(duration)?;
    
    println!("Audio recorded: {} bytes", audio_data.len());
    
    let transcription = transcribe_audio(&api_key, audio_data)?;
    
    println!("\n======================");
    println!("Transcription Result:");
    println!("======================");
    println!("{}", transcription);
    
    Ok(())
}
