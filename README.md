# WhisperDesk

A local-first desktop transcription app built with Tauri 2, React 19, and whisper-rs. Transcribes audio files using OpenAI Whisper models — entirely on-device, no cloud required.

## Features

- **Local transcription** — all processing happens on your machine; nothing is sent to the cloud
- **Metal GPU acceleration** — Apple Silicon M1/M2/M3 Metal backend for fast inference
- **Acceleration control** — choose Auto / CPU / Metal per transcription; graceful CPU fallback with notification
- **Performance tracking** — realtime factor display after each transcription (e.g. "3.4x realtime · Metal · 12.1s")
- **Model manager** — download and manage Whisper model files (tiny, base, small, medium, large-v3)
- **Library** — browse, search, and edit transcripts; starred items and trash
- **Recording** — microphone and system audio capture
- **Real-time streaming** — segments appear as they are transcribed
- **Export** — copy text, export to PDF or plain text

## Tech Stack

- **Frontend:** React 19, Tailwind CSS v4, Zustand, react-i18next, Lucide icons
- **Backend:** Tauri 2, Rust, SQLite (rusqlite), whisper-rs 0.16.0
- **Audio:** Symphonia (decode), Rubato (resample to 16 kHz mono)
- **Inference:** whisper-rs with Metal feature flag (macOS only)

## Requirements

- macOS 12+ (Apple Silicon recommended for Metal acceleration)
- Rust 1.77+, Node.js 20+

## Development

```bash
# Install frontend dependencies
npm install

# Run in development mode (hot reload)
npm run tauri dev

# Build for production
npm run tauri build
```

## Database Migrations

Migrations live in `src-tauri/migrations/` and run automatically on startup:

| Version | Description |
|---------|-------------|
| V001 | Initial schema (transcripts, segments, speakers, models) |
| V002 | FTS5 full-text search |
| V003 | AI prompt templates |
| V004 | Integrations |
| V005 | Export presets |
| V006 | Whisper job tracking |
| V007 | Acceleration stats (backend, realtime factor, wall time) |

## Acceleration Backends

| Backend | Description | Status |
|---------|-------------|--------|
| Auto | Use fastest available (default) | ✅ |
| CPU | Force software inference | ✅ |
| Metal | Apple GPU via Metal | ✅ macOS only |
| CoreML + ANE | Apple Neural Engine | 🔜 Coming soon |

## Project Structure

```
src/                    # React frontend
  components/
    common/             # Layout, Sidebar
    library/            # TranscriptDetail, LibraryList
    settings/           # AccelerationSettings
    transcription/      # DropZone, ModelManager, PerformanceBar
  pages/                # SettingsPage
  stores/               # Zustand stores (settings, transcript, model, recording)
  lib/                  # types.ts

src-tauri/              # Rust backend
  src/
    audio/              # Decode + resample
    commands/           # Tauri command handlers
    database/           # SQLite + migrations
    models/             # Model manager (download, verify, manage)
    transcription/      # WhisperEngine + pipeline + job management
    settings.rs         # AppSettings with AccelerationBackend
    error.rs            # Typed error enum
  migrations/           # SQL migration files
  benches/              # Criterion benchmark suite
```

## License

MIT
