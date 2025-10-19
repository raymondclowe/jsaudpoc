# Wake Word Detection in Rust: Technical Report

## Executive Summary

This report explores wake word detection options for the Rust audio transcription CLI, with focus on low CPU/memory usage suitable for always-on operation. The proposed solution uses a two-stage approach: lightweight local pattern matching followed by Whisper confirmation.

## Wake Word Detection Approaches

### 1. **Frequency-Time Pattern Matching (Spectrogram-Based)** ⭐ RECOMMENDED

**How it works:**
- Convert audio to frequency-time representation (spectrogram)
- Extract mel-frequency cepstral coefficients (MFCCs) or similar features
- Compare with pre-trained template using distance metrics (cosine similarity, DTW)
- Low computational overhead, suitable for always-on detection

**Advantages:**
- Very low CPU/memory usage
- Fast processing (< 1ms per frame)
- No GPU required
- Simple to implement and tune
- Low false positive rate when tuned properly

**Disadvantages:**
- Moderate accuracy (70-85% detection rate)
- Speaker-dependent (works best with training on target speaker)
- Sensitive to background noise

**Implementation complexity:** Medium

### 2. **Template Matching with Cross-Correlation**

**How it works:**
- Store reference audio templates of the wake word
- Compute cross-correlation between incoming audio and templates
- Trigger when correlation exceeds threshold

**Advantages:**
- Extremely simple implementation
- Very low resource usage
- Fast execution

**Disadvantages:**
- Poor accuracy (50-70%)
- Very sensitive to volume, pitch, speed variations
- High false positive rate

**Implementation complexity:** Low

### 3. **Lightweight Neural Network (Porcupine-style)**

**How it works:**
- Small neural network (< 1MB) trained specifically for wake word
- Runs inference on audio frames
- Libraries: Porcupine, Snowboy (discontinued), custom TinyML models

**Advantages:**
- Good accuracy (85-95%)
- Can be speaker-independent
- Decent noise robustness

**Disadvantages:**
- Requires pre-trained models or training infrastructure
- Higher CPU usage than pattern matching
- May require licensing (Porcupine)
- Integration complexity in Rust

**Implementation complexity:** High (requires ML infrastructure)

### 4. **Hidden Markov Models (HMMs)**

**How it works:**
- Traditional speech recognition approach
- Model phoneme sequences for wake word
- Use Viterbi algorithm for decoding

**Advantages:**
- Proven technology
- Good accuracy when tuned
- No GPU needed

**Disadvantages:**
- Complex implementation
- Higher resource usage than simple pattern matching
- Requires training data and phoneme models

**Implementation complexity:** Very High

### 5. **Voice Activity Detection (VAD) + Keyword Spotting**

**How it works:**
- First stage: Detect speech vs silence
- Second stage: Send speech segments to keyword spotter
- Examples: WebRTC VAD + pattern matching

**Advantages:**
- Reduces false triggers from non-speech
- Modular design
- Can save CPU by only processing speech

**Disadvantages:**
- Two-stage complexity
- VAD adds latency
- Still needs keyword spotting algorithm

**Implementation complexity:** Medium-High

## Recommended Approach: Two-Stage Detection

Based on your requirements (low CPU/memory, always-on, confirmation via Whisper), here's the optimal strategy:

### Stage 1: Local Lightweight Detection (MFCC-based Pattern Matching)

1. **Extract MFCC features** from 1-second rolling window
2. **Compare with template** using Dynamic Time Warping (DTW) or cosine similarity
3. **Low threshold** (permissive) to catch all candidates
4. Expected performance:
   - Processing time: < 1ms per frame
   - Memory: < 5MB
   - CPU: < 2% on modern CPU
   - Detection rate: ~75%
   - False positive rate: ~10-15% (acceptable since Stage 2 confirms)

### Stage 2: Whisper Confirmation

1. When Stage 1 triggers, capture 2-second audio segment
2. Send to Whisper for transcription
3. Check if transcription contains wake word
4. This eliminates false positives from Stage 1

### Expected Overall Performance

- **Detection accuracy:** 90-95% (75% Stage 1 × ~95% Stage 2 filtering)
- **False positive rate:** < 1% (Stage 2 filters out Stage 1 false positives)
- **Response latency:** 1-2 seconds (mostly Whisper API call)
- **Resource usage:** Minimal (Stage 1), spike during confirmation (Stage 2)

## Rust Libraries for Implementation

### Audio Processing
- **`rustfft`** - Fast Fourier Transform for frequency analysis
- **`cpal`** - Already used for audio capture
- **`hound`** - WAV file handling (already used)
- **`dasp`** - Digital audio signal processing

### Feature Extraction
- **`mfcc-rust`** or implement custom MFCC extraction
- **`ndarray`** - N-dimensional arrays for signal processing
- **`spectrum-analyzer`** - Frequency spectrum analysis

### Pattern Matching
- **`dtw`** or custom implementation - Dynamic Time Warping
- **`ndarray`** - Distance metrics (cosine similarity, euclidean)

### Alternative: Pre-built Solutions
- **`porcupine-rust`** - Rust bindings for Porcupine (requires license)
- **`vosk-rs`** - Rust bindings for Vosk (heavier, but free)

## Implementation Plan

### Phase 1: Basic MFCC Feature Extraction
```rust
// Pseudo-code structure
1. Capture audio stream (already implemented)
2. Process in 32ms frames with 10ms hop
3. Apply pre-emphasis filter
4. Compute FFT
5. Apply mel filterbank
6. Take log of energies
7. Apply DCT to get MFCCs (keep first 13 coefficients)
```

### Phase 2: Template Creation
```rust
1. Record multiple samples of wake word "computer"
2. Extract MFCC features from each
3. Average or select best template
4. Store as reference pattern
```

### Phase 3: Real-time Matching
```rust
1. Maintain rolling buffer of MFCC features (1 second)
2. Compare with template using DTW or cosine similarity
3. Trigger when distance < threshold
4. Capture extended audio (2 seconds)
5. Send to Whisper for confirmation
```

### Phase 4: Optimization
```rust
1. Tune threshold for optimal detection/false-positive balance
2. Add noise reduction preprocessing
3. Implement energy-based pre-filtering (skip silence)
4. Add hysteresis to prevent multiple triggers
```

## Proof of Concept Code Structure

The PoC implementation includes:

1. **`src/wake_word.rs`** - Wake word detection module
   - MFCC feature extraction
   - Template matching with DTW
   - Threshold-based triggering

2. **`src/audio_buffer.rs`** - Circular buffer for audio
   - Rolling window management
   - Frame extraction

3. **`examples/wake_word_demo.rs`** - Demo application
   - Continuous listening mode
   - Visual feedback on detection
   - Integration with existing transcription

4. **`training/record_samples.rs`** - Template training tool
   - Record multiple wake word samples
   - Generate averaged template
   - Save template to file

## Performance Benchmarks (Estimated)

Based on similar implementations:

| Metric | Value |
|--------|-------|
| Frame processing time | 0.5-1ms |
| Memory usage | 3-5MB |
| CPU usage (idle) | 1-2% |
| CPU usage (processing) | 3-5% |
| Template size | < 50KB |
| Detection latency | 100-500ms |
| False positive rate (Stage 1) | 10-15% |
| False positive rate (after Stage 2) | < 1% |
| True positive rate (overall) | 90-95% |

## Alternative: Using Porcupine (Commercial Solution)

If the MFCC approach doesn't meet accuracy requirements:

```rust
// Using porcupine-rust crate
use porcupine::{Porcupine, PorcupineBuilder};

let porcupine = PorcupineBuilder::new()
    .access_key("YOUR_ACCESS_KEY")
    .keyword_path("computer_linux.ppn")
    .build()
    .expect("Failed to create Porcupine");

// In audio callback
let keyword_index = porcupine.process(&audio_frame)?;
if keyword_index >= 0 {
    println!("Wake word detected!");
    // Trigger transcription
}
```

**Pros:** Better accuracy (95%+), pre-trained models, commercial support
**Cons:** Requires license ($0.10-0.20/device/month or one-time fee), less customizable

## Comparison Matrix

| Approach | CPU | Memory | Accuracy | Implementation | Cost |
|----------|-----|--------|----------|----------------|------|
| MFCC + DTW | Low | Low | 75-85% | Medium | Free |
| Cross-correlation | Very Low | Very Low | 50-70% | Low | Free |
| Porcupine | Low | Low | 95%+ | Low | $$ |
| Vosk | Medium | Medium | 90-95% | Medium | Free |
| Custom NN | Medium | Low | 85-95% | Very High | Free |

## Recommendations

1. **Start with MFCC + DTW approach** (implemented in PoC)
   - Meets low resource requirements
   - Achieves acceptable accuracy with Whisper confirmation
   - Fully free and customizable
   - Good learning opportunity

2. **If accuracy insufficient, upgrade to Porcupine**
   - Easy integration with existing code
   - Professional quality
   - Worth the cost for production use

3. **Avoid building custom neural network**
   - Too complex for this use case
   - Resource requirements may exceed constraints
   - Training infrastructure needed

## Frequency Analysis Approach (As Suggested)

Your intuition about frequency-time graphs is correct! This is essentially spectrogram analysis:

```
Time →
↓ Frequency

[====■■■■====]  "com"
[========■■■]  "pu"
[■■■■========]  "ter"
```

Each phoneme has characteristic frequency patterns. The MFCC approach captures this:
- Mel scale emphasizes human-relevant frequencies (300-3400 Hz)
- Cepstral coefficients capture the "shape" of the frequency distribution
- DTW allows matching despite time variations (speaking speed)

This is exactly what the PoC implements!

## Next Steps

1. ✅ Review this report
2. ⬜ Examine proof of concept code
3. ⬜ Test wake word detection with sample audio
4. ⬜ Train custom template with your voice
5. ⬜ Tune threshold for optimal performance
6. ⬜ Integrate with main application
7. ⬜ Consider Porcupine if accuracy needs improvement

## References

- MFCC: Mel-Frequency Cepstral Coefficients
- DTW: Dynamic Time Warping
- VAD: Voice Activity Detection
- Porcupine: https://picovoice.ai/platform/porcupine/
- Vosk: https://alphacephei.com/vosk/
- "Speech and Language Processing" by Jurafsky & Martin
- "Fundamentals of Speech Recognition" by Rabiner & Juang
