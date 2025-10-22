use std::thread;
use std::time::Duration;
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy)]
pub enum TrekSound {
    ComputerReady,    // TOS-style computer acknowledgement
    CommunicatorChirp // TNG-style communicator sound
}

// Cache for the detected Linux sound command
#[cfg(target_os = "linux")]
static LINUX_SOUND_COMMAND: OnceLock<Option<(&'static str, &'static [&'static str])>> = OnceLock::new();

#[cfg(target_os = "windows")]
fn play_sound(sound_type: TrekSound) {
    use winapi::um::winuser::MessageBeep;
    
    match sound_type {
        TrekSound::ComputerReady => {
            // Use standard system beep for computer ready
            unsafe { MessageBeep(0xFFFFFFFF); } // Simple beep
        }
        TrekSound::CommunicatorChirp => {
            // For communicator, we'll generate a more complex sound on Windows
            generate_communicator_chirp();
        }
    }
}

#[cfg(target_os = "windows")]
fn generate_communicator_chirp() {
    use std::f32::consts::PI;
    use winapi::um::mmsystem::{sndPlaySoundA, SND_MEMORY, SND_ASYNC, SND_NODEFAULT};
    use winapi::ctypes::c_char;
    
    let sample_rate = 44100;
    let duration_ms = 400;
    let num_samples = (sample_rate * duration_ms) / 1000;
    
    // WAV header structure
    let mut wav_data = Vec::new();
    
    // RIFF header
    wav_data.extend(b"RIFF");
    wav_data.extend(&(36 + num_samples * 2).to_le_bytes()); // file size - 8
    wav_data.extend(b"WAVE");
    
    // fmt chunk
    wav_data.extend(b"fmt ");
    wav_data.extend(&16u32.to_le_bytes()); // chunk size
    wav_data.extend(&1u16.to_le_bytes());  // PCM format
    wav_data.extend(&1u16.to_le_bytes());  // mono
    wav_data.extend(&(sample_rate as u32).to_le_bytes()); // sample rate
    wav_data.extend(&((sample_rate * 2) as u32).to_le_bytes()); // byte rate
    wav_data.extend(&2u16.to_le_bytes());  // block align
    wav_data.extend(&16u16.to_le_bytes()); // bits per sample
    
    // data chunk
    wav_data.extend(b"data");
    wav_data.extend(&((num_samples * 2) as u32).to_le_bytes()); // data size
    
    // Generate TNG communicator chirp - more complex sweeping tones
    for i in 0..num_samples {
        let t = i as f32 / sample_rate as f32;
        
        // Create the iconic TNG communicator chirp with multiple components
        let sweep_freq = 800.0 + 400.0 * (t * 8.0).sin(); // Sweeping base frequency
        let chirp_freq = 1200.0 + 800.0 * (t * 12.0).sin(); // Higher chirp component
        let click_freq = if i % 100 < 2 { 2000.0 } else { 0.0 }; // Occasional clicks
        
        // Envelope with sharp attack and decay
        let envelope = if t < 0.05 {
            t / 0.05 // Quick attack
        } else if t < 0.3 {
            1.0 - ((t - 0.05) / 0.25).powi(2) // Gentle decay
        } else {
            (1.0 - (t - 0.3) / 0.1).max(0.0) // Quick release
        };
        
        let sample = (envelope * 0.3 * (
            (2.0 * PI * sweep_freq * t).sin() * 0.5 +
            (2.0 * PI * chirp_freq * t).sin() * 0.3 +
            (2.0 * PI * click_freq * t).sin() * 0.2
        ) * i16::MAX as f32) as i16;
        
        wav_data.extend(&sample.to_le_bytes());
    }
    
    unsafe {
        sndPlaySoundA(wav_data.as_ptr() as *const c_char, SND_MEMORY | SND_ASYNC | SND_NODEFAULT);
    }
    
    // Let the sound play
    thread::sleep(Duration::from_millis(duration_ms as u64));
}

#[cfg(target_os = "linux")]
fn play_sound(sound_type: TrekSound) {
    use std::process::Command;
    
    let (pcm_data, duration_ms) = match sound_type {
        TrekSound::ComputerReady => generate_computer_chime(),
        TrekSound::CommunicatorChirp => generate_communicator_chirp_linux(),
    };
    
    // Get the cached sound command or probe if first time
    let sound_command = LINUX_SOUND_COMMAND.get_or_init(|| {
        probe_linux_sound_command()
    });
    
    if let Some((command, args)) = sound_command {
        let mut child = Command::new(*command)
            .args(*args)
            .stdin(std::process::Stdio::piped())
            .spawn();
            
        if let Ok(mut child_process) = child {
            if let Some(mut stdin) = child_process.stdin.take() {
                use std::io::Write;
                if stdin.write_all(&pcm_data).is_ok() {
                    let _ = child_process.wait(); // Wait for playback to complete
                    return;
                }
            }
            let _ = child_process.kill(); // Clean up if failed
        }
    }
    
    // Fallback: just print a message if no sound command worked
    println!("\x07"); // Terminal bell as last resort
    thread::sleep(Duration::from_millis(duration_ms));
}

#[cfg(target_os = "linux")]
fn probe_linux_sound_command() -> Option<(&'static str, &'static [&'static str])> {
    use std::process::Command;
    
    // Test commands in order of preference
    let sound_commands = [
        // Try paplay first (PulseAudio)
        ("paplay", &["--rate=44100", "--channels=1", "--format=s16le"] as &[&str]),
        // Try aplay (ALSA)
        ("aplay", &["-q", "-r", "44100", "-c", "1", "-f", "S16_LE"]),
        // Try play (SoX)
        ("play", &["-q", "-r", "44100", "-c", "1", "-e", "signed-integer", "-b", "16", "-t", "raw", "-"]),
    ];
    
    for &(command, args) in &sound_commands {
        // Test if the command exists and works by running it with --help or --version
        let test = Command::new(command)
            .arg("--help")
            .output()
            .or_else(|_| Command::new(command).arg("--version").output())
            .or_else(|_| Command::new(command).output());
            
        if test.is_ok() {
            println!("[DEBUG] Using sound command: {}", command);
            return Some((command, args));
        }
    }
    
    println!("[DEBUG] No sound command found, using terminal bell fallback");
    None
}

#[cfg(target_os = "linux")]
fn generate_computer_chime() -> (Vec<u8>, u64) {
    use std::f32::consts::PI;
    
    let sample_rate = 44100;
    let duration_ms = 200;
    let num_samples = (sample_rate * duration_ms) / 1000;
    
    let mut pcm_data = Vec::new();
    for i in 0..num_samples {
        let t = i as f32 / sample_rate as f32;
        // Create a pleasant two-tone chime
        let freq1 = 440.0; // A4
        let freq2 = 660.0; // E5
        let volume = 0.3 * (1.0 - (i as f32 / num_samples as f32)).powi(2); // Fade out
        let sample = (volume * (
            (2.0 * PI * freq1 * t).sin() * 0.6 +
            (2.0 * PI * freq2 * t).sin() * 0.4
        ) * i16::MAX as f32) as i16;
        
        pcm_data.extend_from_slice(&sample.to_le_bytes());
    }
    
    (pcm_data, duration_ms as u64)
}

#[cfg(target_os = "linux")]
fn generate_communicator_chirp_linux() -> (Vec<u8>, u64) {
    use std::f32::consts::PI;
    
    let sample_rate = 44100;
    let duration_ms = 400;
    let num_samples = (sample_rate * duration_ms) / 1000;
    
    let mut pcm_data = Vec::new();
    
    // Generate TNG communicator chirp - more complex sweeping tones
    for i in 0..num_samples {
        let t = i as f32 / sample_rate as f32;
        
        // Create the iconic TNG communicator chirp with multiple components
        let sweep_freq = 800.0 + 400.0 * (t * 8.0).sin(); // Sweeping base frequency
        let chirp_freq = 1200.0 + 800.0 * (t * 12.0).sin(); // Higher chirp component
        let click_freq = if i % 100 < 2 { 2000.0 } else { 0.0 }; // Occasional clicks
        
        // Envelope with sharp attack and decay
        let envelope = if t < 0.05 {
            t / 0.05 // Quick attack
        } else if t < 0.3 {
            1.0 - ((t - 0.05) / 0.25).powi(2) // Gentle decay
        } else {
            (1.0 - (t - 0.3) / 0.1).max(0.0) // Quick release
        };
        
        let sample = (envelope * 0.3 * (
            (2.0 * PI * sweep_freq * t).sin() * 0.5 +
            (2.0 * PI * chirp_freq * t).sin() * 0.3 +
            (2.0 * PI * click_freq * t).sin() * 0.2
        ) * i16::MAX as f32) as i16;
        
        pcm_data.extend_from_slice(&sample.to_le_bytes());
    }
    
    (pcm_data, duration_ms as u64)
}

fn main() {
    println!("=== Star Trek Sound Demo ===");
    
    // Demo computer ready sound
    println!("\nCaptain: Computer...");
    thread::sleep(Duration::from_millis(1000));
    println!("*TOS-style computer acknowledgement chime*");
    play_sound(TrekSound::ComputerReady);
    thread::sleep(Duration::from_millis(500));
    
    // Demo communicator chirp
    println!("\nCaptain: Picard to Enterprise...");
    thread::sleep(Duration::from_millis(800));
    println!("*TNG-style communicator chirp*");
    play_sound(TrekSound::CommunicatorChirp);
    thread::sleep(Duration::from_millis(500));
    
    // Show that subsequent calls use the cached command
    println!("\nFirst Officer: Computer, status report...");
    thread::sleep(Duration::from_millis(800));
    println!("*TOS-style computer acknowledgement chime*");
    play_sound(TrekSound::ComputerReady);
    
    println!("\n=== End of demo ===");
}

// Convenience functions for easy use
pub fn computer_ready() {
    play_sound(TrekSound::ComputerReady);
}

pub fn communicator_chirp() {
    play_sound(TrekSound::CommunicatorChirp);
}