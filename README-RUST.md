# Audio Transcription CLI (Rust)

A command-line tool for recording audio and transcribing it using Replicate's Whisper API.

## Features

- Records audio from default microphone
- Sends audio to Replicate for transcription using Whisper
- Displays transcribed text to stdout
- Reads API key from `.env` file
- Cross-platform (Linux, Windows, macOS)

## Prerequisites

- Rust toolchain (install from https://rustup.rs/)
- Replicate API key (get from https://replicate.com/account/api-tokens)

## Setup

1. Clone the repository
2. Copy `.env.example` to `.env`:
   ```bash
   cp .env.example .env
   ```
3. Edit `.env` and add your Replicate API key:
   ```
   REPLICATE_API_KEY=your_actual_api_key_here
   ```

## Building

### For Linux
```bash
cargo build --release
```

The binary will be at `target/release/audio-transcribe-cli`

### For Windows (cross-compile from Linux)
```bash
# Install Windows target
rustup target add x86_64-pc-windows-gnu

# Build for Windows
cargo build --release --target x86_64-pc-windows-gnu
```

The binary will be at `target/x86_64-pc-windows-gnu/release/audio-transcribe-cli.exe`

### For macOS (cross-compile from Linux)
```bash
# Install macOS target
rustup target add x86_64-apple-darwin

# Build for macOS (requires osxcross or similar)
cargo build --release --target x86_64-apple-darwin
```

## Running

```bash
# Run directly with cargo
cargo run

# Or run the built binary
./target/release/audio-transcribe-cli
```

By default, it records 5 seconds of audio. You can change this by setting `RECORD_DURATION` in `.env`:

```
RECORD_DURATION=10
```

## How It Works

1. Loads `REPLICATE_API_KEY` from `.env` file
2. Records audio from the default input device for specified duration (default: 5 seconds)
3. Converts audio to WAV format
4. Sends audio to Replicate's Whisper API
5. Displays the transcribed text

## Dependencies

- `cpal` - Cross-platform audio I/O
- `hound` - WAV encoding/decoding
- `reqwest` - HTTP client
- `serde` / `serde_json` - JSON serialization
- `dotenv` - Environment variable management
- `anyhow` - Error handling

## Troubleshooting

### No audio device found
Make sure your microphone is connected and set as the default input device.

### API errors
- Check that your `REPLICATE_API_KEY` is correct
- Ensure you have credits in your Replicate account
- Check your internet connection

### Build errors on Windows
If cross-compiling to Windows fails, you may need to install MinGW:
```bash
sudo apt-get install mingw-w64
```
