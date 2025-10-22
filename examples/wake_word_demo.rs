/// Wake Word Detection Demo
/// 
/// This example demonstrates how to use the wake word detection module.
/// It shows:
/// 1. Training a template from sample audio
/// 2. Continuous monitoring for wake word detection
/// 3. Integration with the existing transcription system

use anyhow::{Context, Result};
use audio_transcribe_cli::wake_word::WakeWordDetector;
use std::fs;
use std::path::Path;

fn main() -> Result<()> {
    println!("Wake Word Detection Demo");
    println!("========================\n");
    
    // Demo 1: Create a simple synthetic audio for testing
    println!("Demo 1: Testing with synthetic audio");
    demo_synthetic_audio()?;
    
    println!("\n");
    
    // Demo 2: Show how to train from WAV files
    println!("Demo 2: Training template from samples");
    demo_template_training()?;
    
    println!("\n");
    
    // Demo 3: Show detection on test audio
    println!("Demo 3: Wake word detection");
    demo_detection()?;
    
    Ok(())
}

/// Demo 1: Test with synthetic audio
fn demo_synthetic_audio() -> Result<()> {
    let mut detector = WakeWordDetector::new();
    
    // Create a simple synthetic "wake word" pattern
    // This is a sequence with specific frequency characteristics
    let sample_rate = 16000;
    let duration = 1.0; // 1 second
    
    // Generate a chirp signal (frequency sweep) as a synthetic wake word
    let samples: Vec<f32> = (0..(sample_rate as f32 * duration) as usize)
        .map(|i| {
            let t = i as f32 / sample_rate as f32;
            // Sweep from 300 Hz to 1500 Hz
            let freq = 300.0 + (1200.0 * t);
            let phase = 2.0 * std::f32::consts::PI * freq * t;
            phase.sin() * 0.5 // Amplitude of 0.5
        })
        .collect();
    
    println!("  Generated {} samples of synthetic audio", samples.len());
    
    // Extract MFCC features
    let mfcc = detector.extract_mfcc(&samples)?;
    println!("  Extracted MFCC features: {} frames × {} coefficients", 
             mfcc.nrows(), mfcc.ncols());
    
    // Set this as the template
    detector.set_template(mfcc.clone());
    
    // Test detection on the same audio (should detect with high confidence)
    let (detected, confidence) = detector.detect(&samples)?;
    println!("  Detection result: {} (confidence: {:.2}%)", 
             if detected { "✓ DETECTED" } else { "✗ NOT DETECTED" },
             confidence * 100.0);
    
    // Test on different audio (should not detect)
    let noise: Vec<f32> = (0..sample_rate)
        .map(|i| (i as f32 * 0.001).sin() * 0.1) // Different pattern
        .collect();
    let (detected, confidence) = detector.detect(&noise)?;
    println!("  Detection on noise: {} (confidence: {:.2}%)",
             if detected { "✓ DETECTED" } else { "✗ NOT DETECTED" },
             confidence * 100.0);
    
    Ok(())
}

/// Demo 2: Train template from multiple samples
fn demo_template_training() -> Result<()> {
    let mut detector = WakeWordDetector::new();
    
    // Create multiple variations of the wake word with slight differences
    let sample_rate = 16000;
    let mut samples = Vec::new();
    
    for variation in 0..3 {
        let duration = 1.0 + (variation as f32 * 0.1); // Slightly different durations
        let pitch_shift = 1.0 + (variation as f32 * 0.05); // Slightly different pitches
        
        let sample: Vec<f32> = (0..(sample_rate as f32 * duration) as usize)
            .map(|i| {
                let t = i as f32 / sample_rate as f32;
                let freq = (300.0 + 1200.0 * t) * pitch_shift;
                let phase = 2.0 * std::f32::consts::PI * freq * t;
                phase.sin() * 0.5
            })
            .collect();
        
        samples.push(sample);
    }
    
    println!("  Created {} training samples", samples.len());
    
    // Train the template
    let sample_refs: Vec<Vec<f32>> = samples.into_iter().collect();
    detector.train_template(&sample_refs)?;
    
    println!("  ✓ Template trained successfully");
    
    // Test detection
    let test_audio: Vec<f32> = (0..sample_rate)
        .map(|i| {
            let t = i as f32 / sample_rate as f32;
            let freq = 300.0 + 1200.0 * t;
            let phase = 2.0 * std::f32::consts::PI * freq * t;
            phase.sin() * 0.5
        })
        .collect();
    
    let (detected, confidence) = detector.detect(&test_audio)?;
    println!("  Detection on similar audio: {} (confidence: {:.2}%)",
             if detected { "✓ DETECTED" } else { "✗ NOT DETECTED" },
             confidence * 100.0);
    
    Ok(())
}

/// Demo 3: Show detection workflow
fn demo_detection() -> Result<()> {
    let mut detector = WakeWordDetector::new();
    
    // Setup: Train a template
    let sample_rate = 16000;
    let training_audio: Vec<f32> = (0..sample_rate)
        .map(|i| {
            let t = i as f32 / sample_rate as f32;
            // Multi-tone signal representing "computer"
            (440.0 * t * 2.0 * std::f32::consts::PI).sin() * 0.3 +
            (880.0 * t * 2.0 * std::f32::consts::PI).sin() * 0.3 +
            (1320.0 * t * 2.0 * std::f32::consts::PI).sin() * 0.2
        })
        .collect();
    
    detector.train_template(&[training_audio.clone()])?;
    println!("  ✓ Template trained");
    
    // Adjust threshold for sensitivity
    detector.set_threshold(0.6); // Lower = more sensitive
    println!("  Detection threshold set to 0.6");
    
    // Simulate continuous monitoring
    println!("\n  Simulating continuous audio stream...");
    
    // Test with matching audio
    println!("  - Testing with wake word audio...");
    let (detected, confidence) = detector.detect(&training_audio)?;
    println!("    Result: {} (confidence: {:.2}%)",
             if detected { "✓ WAKE WORD DETECTED!" } else { "✗ Not detected" },
             confidence * 100.0);
    
    // Test with non-matching audio
    println!("  - Testing with background noise...");
    let noise: Vec<f32> = (0..sample_rate)
        .map(|_| rand::random::<f32>() * 0.1 - 0.05)
        .collect();
    let (detected, confidence) = detector.detect(&noise)?;
    println!("    Result: {} (confidence: {:.2}%)",
             if detected { "✓ WAKE WORD DETECTED!" } else { "✗ Not detected" },
             confidence * 100.0);
    
    // Test with partially matching audio
    println!("  - Testing with similar but different audio...");
    let similar: Vec<f32> = (0..sample_rate)
        .map(|i| {
            let t = i as f32 / sample_rate as f32;
            (550.0 * t * 2.0 * std::f32::consts::PI).sin() * 0.5
        })
        .collect();
    let (detected, confidence) = detector.detect(&similar)?;
    println!("    Result: {} (confidence: {:.2}%)",
             if detected { "✓ WAKE WORD DETECTED!" } else { "✗ Not detected" },
             confidence * 100.0);
    
    Ok(())
}

/// Helper to use rand without adding dependency
mod rand {
    static mut SEED: u32 = 12345;
    
    pub fn random<T: From<f32>>() -> T {
        unsafe {
            SEED = SEED.wrapping_mul(1103515245).wrapping_add(12345);
            T::from((SEED / 65536 % 32768) as f32 / 32768.0)
        }
    }
}
