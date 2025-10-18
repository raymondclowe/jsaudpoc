- Replicate's HTTP API rejects `POST /v1/files` uploads built by hand unless the multipart part is named `content`; using the official `replicate` client sidesteps this entirely.
- Passing a raw audio `Buffer` to `replicate.run` automatically uploads the clip and returns transcription output, which is simpler than juggling temporary file URLs.


## Why server.js is needed (vs direct API call from index.html)

- Browser cannot securely store API keys; exposing them risks abuse
- Replicate API requires authentication via secret key, which must not be exposed client-side
- CORS restrictions: Replicate API may block direct browser requests for security
- server.js acts as a backend proxy, safely handling keys and requests
- server can process files, manage uploads, and format responses before sending to browser

## Architecture Change Analysis (2025-10-18)

- HTML/JavaScript web approach is overengineered for core use case (record audio → transcribe → display)
- Command-line compiled tool offers: simpler deployment, faster execution, automation capability, embeddability
- Compared C, C++, C#, and Rust for CLI development:
  - **C**: Too low-level, manual memory management, platform-specific audio APIs, slow prototyping (Score: 3/10)
  - **C++**: Better than C but still verbose, complex build systems, challenging cross-compilation (Score: 6/10)
  - **C#**: Excellent productivity, good cross-platform support via .NET, larger binaries, strong alternative (Score: 7.5/10)
  - **Rust**: Best overall - memory safety without GC, excellent cross-compilation, modern tooling (cargo), mature audio library (cpal), production-ready (Score: 9/10)
- **Recommendation: Rust** for optimal balance of safety, performance, cross-platform support, and long-term maintainability
- Key Rust advantages:
  - `cpal` crate provides excellent cross-platform audio recording
  - `reqwest` + `serde` for HTTP/JSON (equivalent to C#'s ease)
  - `cargo` makes dependency management and cross-compilation trivial
  - Compile-time memory safety prevents entire classes of bugs
  - Easy future expansion to web server mode with `axum` or `actix-web`
- C# strong alternative if faster initial development needed (familiar syntax, Visual Studio tooling, ASP.NET Core)
- Avoid C/C++ for this project: complexity and development time outweigh any benefits

## Rust CLI Implementation (2025-10-18)

- Created command-line Rust tool for audio transcription using Replicate API
- Uses `cpal` for cross-platform audio recording from default microphone
- Uses `hound` to encode audio as WAV format
- Uses `reqwest` with blocking client for HTTP API calls to Replicate
- Secrets stored in `.env` file loaded with `dotenv` crate
- Cross-compilation ready: supports Linux and Windows via `x86_64-pc-windows-gnu` target
- Key implementation details:
  - `hound::WavWriter` requires seekable writer, used temporary file approach
  - Audio recorded to `/tmp/recording.wav` then read into memory for upload
  - Multipart form upload with `reqwest::blocking::multipart`
  - JSON parsing with `serde_json::Value` for flexible response handling
  - Default 5 second recording duration, configurable via `RECORD_DURATION` env var
- Build process:
  - `cargo build --release` for Linux binary
  - `rustup target add x86_64-pc-windows-gnu` to add Windows target
  - `cargo build --release --target x86_64-pc-windows-gnu` for Windows binary
  - Requires `mingw-w64` package for Windows cross-compilation

