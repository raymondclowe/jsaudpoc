- Replicate's HTTP API rejects `POST /v1/files` uploads built by hand unless the multipart part is named `content`; using the official `replicate` client sidesteps this entirely.
- Passing a raw audio `Buffer` to `replicate.run` automatically uploads the clip and returns transcription output, which is simpler than juggling temporary file URLs.


## Why server.js is needed (vs direct API call from index.html)

- Browser cannot securely store API keys; exposing them risks abuse
- Replicate API requires authentication via secret key, which must not be exposed client-side
- CORS restrictions: Replicate API may block direct browser requests for security
- server.js acts as a backend proxy, safely handling keys and requests
- server can process files, manage uploads, and format responses before sending to browser
