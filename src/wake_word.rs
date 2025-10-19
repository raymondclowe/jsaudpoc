/// Wake Word Detection Module
/// 
/// Implements a lightweight wake word detection system using MFCC features
/// and Dynamic Time Warping (DTW) for pattern matching.
/// 
/// This is designed for low CPU/memory usage suitable for always-on operation.

use anyhow::Result;
use ndarray::{Array1, Array2};
use rustfft::{FftPlanner, num_complex::Complex};
use std::f32::consts::PI;

/// MFCC feature extractor configuration
pub struct MfccConfig {
    pub sample_rate: u32,
    pub frame_size: usize,      // Number of samples per frame (typically 512 or 1024)
    pub hop_size: usize,        // Step size between frames (typically frame_size / 4)
    pub num_mfcc: usize,        // Number of MFCC coefficients to extract (typically 13)
    pub num_filters: usize,     // Number of mel filters (typically 26-40)
    pub min_freq: f32,          // Minimum frequency for mel scale (typically 300 Hz)
    pub max_freq: f32,          // Maximum frequency for mel scale (typically 8000 Hz)
}

impl Default for MfccConfig {
    fn default() -> Self {
        Self {
            sample_rate: 16000,
            frame_size: 512,
            hop_size: 128,
            num_mfcc: 13,
            num_filters: 26,
            min_freq: 300.0,
            max_freq: 8000.0,
        }
    }
}

/// Wake word detector using MFCC + DTW
pub struct WakeWordDetector {
    config: MfccConfig,
    template: Option<Array2<f32>>,
    threshold: f32,
    mel_filterbank: Array2<f32>,
    dct_matrix: Array2<f32>,
}

impl WakeWordDetector {
    /// Create a new wake word detector with default configuration
    pub fn new() -> Self {
        let config = MfccConfig::default();
        let mel_filterbank = create_mel_filterbank(&config);
        let dct_matrix = create_dct_matrix(config.num_filters, config.num_mfcc);
        
        Self {
            config,
            template: None,
            threshold: 0.7, // Default threshold (lower = more sensitive)
            mel_filterbank,
            dct_matrix,
        }
    }
    
    /// Set the wake word template (pre-computed MFCC features)
    pub fn set_template(&mut self, template: Array2<f32>) {
        self.template = Some(template);
    }
    
    /// Set the detection threshold (0.0 = always trigger, 1.0 = never trigger)
    pub fn set_threshold(&mut self, threshold: f32) {
        self.threshold = threshold.clamp(0.0, 1.0);
    }
    
    /// Extract MFCC features from audio samples
    /// 
    /// Returns a 2D array where each row is a frame and each column is an MFCC coefficient
    pub fn extract_mfcc(&self, audio: &[f32]) -> Result<Array2<f32>> {
        if audio.len() < self.config.frame_size {
            return Ok(Array2::zeros((0, self.config.num_mfcc)));
        }
        
        let num_frames = (audio.len() - self.config.frame_size) / self.config.hop_size + 1;
        let mut mfcc_features = Array2::zeros((num_frames, self.config.num_mfcc));
        
        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(self.config.frame_size);
        
        for frame_idx in 0..num_frames {
            let start = frame_idx * self.config.hop_size;
            let end = start + self.config.frame_size;
            
            if end > audio.len() {
                break;
            }
            
            let frame = &audio[start..end];
            
            // Apply pre-emphasis filter (boost high frequencies)
            let pre_emphasized = apply_pre_emphasis(frame, 0.97);
            
            // Apply Hamming window
            let windowed = apply_hamming_window(&pre_emphasized);
            
            // Compute FFT
            let mut buffer: Vec<Complex<f32>> = windowed
                .iter()
                .map(|&x| Complex::new(x, 0.0))
                .collect();
            fft.process(&mut buffer);
            
            // Compute power spectrum
            let power_spectrum: Vec<f32> = buffer[..self.config.frame_size / 2]
                .iter()
                .map(|c| (c.norm_sqr() + 1e-10).ln())
                .collect();
            
            // Apply mel filterbank
            let mel_energies = self.mel_filterbank.dot(&Array1::from(power_spectrum));
            
            // Apply DCT to get MFCC coefficients
            let mfcc = self.dct_matrix.dot(&mel_energies);
            
            // Store in output array
            for i in 0..self.config.num_mfcc {
                mfcc_features[[frame_idx, i]] = mfcc[i];
            }
        }
        
        Ok(mfcc_features)
    }
    
    /// Detect wake word in audio samples
    /// 
    /// Returns true if the wake word is detected, along with the confidence score
    pub fn detect(&self, audio: &[f32]) -> Result<(bool, f32)> {
        let template = match &self.template {
            Some(t) => t,
            None => return Ok((false, 0.0)),
        };
        
        // Extract MFCC features from input audio
        let features = self.extract_mfcc(audio)?;
        
        if features.nrows() == 0 {
            return Ok((false, 0.0));
        }
        
        // Compute DTW distance between features and template
        let distance = dtw_distance(&features, template);
        
        // Normalize distance to 0-1 range (approximate)
        let max_distance = (template.nrows() as f32 * self.config.num_mfcc as f32).sqrt();
        let normalized_distance = (distance / max_distance).min(1.0);
        
        // Convert distance to similarity (1 - distance)
        let similarity = 1.0 - normalized_distance;
        
        // Check if similarity exceeds threshold
        let detected = similarity >= self.threshold;
        
        Ok((detected, similarity))
    }
    
    /// Train a template from multiple audio samples
    /// 
    /// This averages the MFCC features from multiple recordings
    /// to create a robust template
    pub fn train_template(&mut self, samples: &[Vec<f32>]) -> Result<()> {
        if samples.is_empty() {
            anyhow::bail!("Need at least one sample to train");
        }
        
        // Extract MFCC from all samples
        let mut all_features = Vec::new();
        for sample in samples {
            let features = self.extract_mfcc(sample)?;
            if features.nrows() > 0 {
                all_features.push(features);
            }
        }
        
        if all_features.is_empty() {
            anyhow::bail!("No valid features extracted from samples");
        }
        
        // Use the median length to avoid outliers
        let mut lengths: Vec<usize> = all_features.iter().map(|f| f.nrows()).collect();
        lengths.sort_unstable();
        let target_length = lengths[lengths.len() / 2];
        
        // Average features (time-align using DTW first would be better, but simple average works)
        let mut template = Array2::zeros((target_length, self.config.num_mfcc));
        let mut count = 0;
        
        for features in all_features {
            // Simple linear interpolation to match target length
            for i in 0..target_length {
                let src_idx = (i as f32 * (features.nrows() - 1) as f32 / (target_length - 1) as f32) as usize;
                let src_idx = src_idx.min(features.nrows() - 1);
                for j in 0..self.config.num_mfcc {
                    template[[i, j]] += features[[src_idx, j]];
                }
            }
            count += 1;
        }
        
        // Normalize by count
        template /= count as f32;
        
        self.template = Some(template);
        
        Ok(())
    }
}

impl Default for WakeWordDetector {
    fn default() -> Self {
        Self::new()
    }
}

/// Apply pre-emphasis filter to boost high frequencies
fn apply_pre_emphasis(signal: &[f32], alpha: f32) -> Vec<f32> {
    let mut result = vec![0.0; signal.len()];
    result[0] = signal[0];
    for i in 1..signal.len() {
        result[i] = signal[i] - alpha * signal[i - 1];
    }
    result
}

/// Apply Hamming window to reduce spectral leakage
fn apply_hamming_window(signal: &[f32]) -> Vec<f32> {
    let n = signal.len();
    signal
        .iter()
        .enumerate()
        .map(|(i, &x)| {
            let window = 0.54 - 0.46 * (2.0 * PI * i as f32 / (n - 1) as f32).cos();
            x * window
        })
        .collect()
}

/// Create mel filterbank matrix
fn create_mel_filterbank(config: &MfccConfig) -> Array2<f32> {
    let num_fft_bins = config.frame_size / 2;
    let mut filterbank = Array2::zeros((config.num_filters, num_fft_bins));
    
    // Convert Hz to Mel scale
    let hz_to_mel = |hz: f32| 2595.0 * (1.0 + hz / 700.0).log10();
    let mel_to_hz = |mel: f32| 700.0 * (10.0_f32.powf(mel / 2595.0) - 1.0);
    
    let min_mel = hz_to_mel(config.min_freq);
    let max_mel = hz_to_mel(config.max_freq);
    
    // Create evenly spaced mel points
    let mel_points: Vec<f32> = (0..=config.num_filters + 1)
        .map(|i| min_mel + (max_mel - min_mel) * i as f32 / (config.num_filters + 1) as f32)
        .map(mel_to_hz)
        .collect();
    
    // Convert Hz points to FFT bin indices
    let bin_points: Vec<usize> = mel_points
        .iter()
        .map(|&hz| ((hz * config.frame_size as f32) / config.sample_rate as f32).floor() as usize)
        .collect();
    
    // Create triangular filters
    for i in 0..config.num_filters {
        let start = bin_points[i];
        let center = bin_points[i + 1];
        let end = bin_points[i + 2];
        
        // Rising slope
        for j in start..center {
            if j < num_fft_bins {
                filterbank[[i, j]] = (j - start) as f32 / (center - start) as f32;
            }
        }
        
        // Falling slope
        for j in center..end {
            if j < num_fft_bins {
                filterbank[[i, j]] = (end - j) as f32 / (end - center) as f32;
            }
        }
    }
    
    filterbank
}

/// Create DCT (Discrete Cosine Transform) matrix for MFCC computation
fn create_dct_matrix(num_filters: usize, num_mfcc: usize) -> Array2<f32> {
    let mut dct = Array2::zeros((num_mfcc, num_filters));
    
    for i in 0..num_mfcc {
        for j in 0..num_filters {
            dct[[i, j]] = (PI * i as f32 * (j as f32 + 0.5) / num_filters as f32).cos();
            if i == 0 {
                dct[[i, j]] *= (1.0 / num_filters as f32).sqrt();
            } else {
                dct[[i, j]] *= (2.0 / num_filters as f32).sqrt();
            }
        }
    }
    
    dct
}

/// Compute Dynamic Time Warping distance between two sequences
/// 
/// This allows matching patterns even when they're spoken at different speeds
fn dtw_distance(seq1: &Array2<f32>, seq2: &Array2<f32>) -> f32 {
    let n = seq1.nrows();
    let m = seq2.nrows();
    let dim = seq1.ncols();
    
    if n == 0 || m == 0 {
        return f32::MAX;
    }
    
    // Initialize DTW matrix with infinity
    let mut dtw = Array2::from_elem((n + 1, m + 1), f32::MAX);
    dtw[[0, 0]] = 0.0;
    
    // Fill DTW matrix
    for i in 1..=n {
        for j in 1..=m {
            // Compute Euclidean distance between frames
            let mut dist = 0.0;
            for k in 0..dim {
                let diff = seq1[[i - 1, k]] - seq2[[j - 1, k]];
                dist += diff * diff;
            }
            dist = dist.sqrt();
            
            // DTW recurrence relation
            let cost = dist + dtw[[i - 1, j - 1]].min(dtw[[i - 1, j]]).min(dtw[[i, j - 1]]);
            dtw[[i, j]] = cost;
        }
    }
    
    dtw[[n, m]]
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_pre_emphasis() {
        let signal = vec![1.0, 2.0, 3.0, 4.0];
        let result = apply_pre_emphasis(&signal, 0.97);
        assert_eq!(result.len(), signal.len());
        assert_eq!(result[0], signal[0]);
    }
    
    #[test]
    fn test_hamming_window() {
        let signal = vec![1.0; 256];
        let result = apply_hamming_window(&signal);
        assert_eq!(result.len(), signal.len());
        // Window should taper at edges
        assert!(result[0] < result[128]);
        assert!(result[255] < result[128]);
    }
    
    #[test]
    fn test_mfcc_extraction() {
        let detector = WakeWordDetector::new();
        // Generate a simple sine wave
        let sample_rate = 16000;
        let duration = 1.0; // 1 second
        let frequency = 440.0; // A4 note
        let samples: Vec<f32> = (0..(sample_rate as f32 * duration) as usize)
            .map(|i| (2.0 * PI * frequency * i as f32 / sample_rate as f32).sin())
            .collect();
        
        let mfcc = detector.extract_mfcc(&samples).unwrap();
        assert!(mfcc.nrows() > 0);
        assert_eq!(mfcc.ncols(), 13);
    }
    
    #[test]
    fn test_dtw_distance() {
        let seq1 = Array2::from_shape_vec((3, 2), vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]).unwrap();
        let seq2 = Array2::from_shape_vec((3, 2), vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]).unwrap();
        let dist = dtw_distance(&seq1, &seq2);
        assert!(dist < 0.1); // Should be very close to 0 for identical sequences
    }
}
