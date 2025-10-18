const express = require('express');
const multer = require('multer');
const Replicate = require('replicate');

const app = express();
const upload = multer();

require('dotenv').config();
const API_KEY = process.env.REPLICATE_API_KEY;
const WHISPER_VERSION = 'vaibhavs10/incredibly-fast-whisper:3ab86df6c8f54c11309d4d1f930ac292bad43ace52d10c80d87eb258b3c9f79c';

const replicate = new Replicate({
  auth: API_KEY,
  fileEncodingStrategy: 'upload',
});

app.use(express.static('.')); // Serve static files from current directory
app.use(express.json({ limit: '10mb' }));

app.get('/test', async (req, res) => {
  try {
    const response = await fetch('https://api.replicate.com/v1/models/openai/whisper', {
      headers: {
        'Authorization': `Bearer ${API_KEY}`
      }
    });
    const text = await response.text();
    res.json({ status: response.status, text });
  } catch (error) {
    res.json({ error: error.message });
  }
});

app.post('/transcribe', upload.single('file'), async (req, res) => {
  try {
    const { audioUrl } = req.body;

    if (req.file) {
      const prediction = await replicate.run(WHISPER_VERSION, {
        input: { audio: req.file.buffer },
        wait: { mode: 'block' },
      });

      const text = prediction?.text || '';
      res.json({ text, chunks: prediction?.chunks || [] });
      return;
    }

    if (!audioUrl) {
      throw new Error('No audio provided for transcription');
    }

    const prediction = await replicate.predictions.create({
      version: WHISPER_VERSION,
      input: { audio: audioUrl },
      wait: true,
    });

    const text = prediction?.output?.text
      || (Array.isArray(prediction?.output) ? prediction.output.join('\n') : '');

    res.json({ text, chunks: prediction?.output?.chunks || [] });
  } catch (error) {
    console.error('Transcribe error:', error);
    res.status(500).json({ error: error.message });
  }
});

const PORT = 3000;
app.listen(PORT, () => {
  console.log(`Server running on http://localhost:${PORT}`);
});

process.on('unhandledRejection', (reason, promise) => {
  console.error('Unhandled Rejection at:', promise, 'reason:', reason);
});