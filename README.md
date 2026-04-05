# WhisperDesk

A local-first desktop transcription app built with Tauri 2, React 19, and whisper-rs. Transcribes audio files using OpenAI Whisper models — entirely on-device, no cloud required.

## Features

- **Local transcription** — all processing happens on your machine; nothing is sent to the cloud
- **Metal GPU acceleration** — Apple Silicon M1/M2/M3 Metal backend for fast inference
- **Acceleration control** — choose Auto / CPU / Metal per transcription; graceful CPU fallback with notification
- **Performance tracking** — realtime factor display after each transcription (e.g. "3.4x realtime · Metal · 12.1s")
- **Model manager** — download and manage Whisper model files (tiny, base, small, medium, large-v3)
- **Waveform editor** — wavesurfer.js powered waveform display with play/pause/seek and 0.5x-3x speed control
- **Click-to-seek** — click any word in the transcript to jump to that audio position; active segment highlighted
- **Segment editing** — inline edit, merge (Cmd+J), split at cursor, and delete segments with undo support
- **Undo/redo** — 50-operation undo stack for all edit operations (Cmd+Z / Cmd+Shift+Z)
- **Find and replace** — Cmd+F search within transcript with case-sensitive toggle and replace all
- **Compact mode** — toggle timestamps on/off for dense text-only view (Cmd+Shift+C)
- **FTS5 search** — full-text search across all transcripts with highlighted match excerpts
- **Library** — browse, search, sort, and filter transcripts; starred items and trash with 30-day recovery
- **Folders and tags** — organize transcripts into folders and assign tags; filter by folder/tag
- **Smart folders** — auto-populating folders based on filter criteria
- **Export** — TXT, SRT, VTT subtitle formats with speaker labels and timestamps
- **WhisperDesk archive** — `.whisper` ZIP+JSON format for lossless transcript export/import with audio
- **Copy to clipboard** — copy full transcript or selected segments
- **Video player** — inline video playback with synced subtitle overlay for video source files
- **Recording** — microphone and system audio capture
- **Real-time streaming** — segments appear as they are transcribed
- **Export dialog** — format picker, option toggles, destination picker with preview

## Tech Stack

- **Frontend:** React 19, Tailwind CSS v4, Zustand, react-i18next, Lucide icons, wavesurfer.js
- **Backend:** Tauri 2, Rust, SQLite (rusqlite), whisper-rs 0.16.0
- **Audio:** Symphonia (decode), Rubato (resample to 16 kHz mono)
- **Inference:** whisper-rs with Metal feature flag (macOS only)
- **Export:** SRT, VTT, TXT, ZIP-based .whisper archive

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
| V008 | Smart folders (id, name, filter_json) |
| V009 | FTS index population for existing segments |

## Acceleration Backends

| Backend | Description | Status |
|---------|-------------|--------|
| Auto | Use fastest available (default) | Supported |
| CPU | Force software inference | Supported |
| Metal | Apple GPU via Metal | Supported (macOS only) |
| CoreML + ANE | Apple Neural Engine | Coming soon |

## Export Formats

| Format | Description | Features |
|--------|-------------|----------|
| TXT | Plain text | Timestamps, speaker labels |
| SRT | SubRip subtitle | Millisecond timestamps, speaker tags |
| VTT | WebVTT subtitle | Millisecond timestamps, speaker tags |
| .whisper | WhisperDesk archive | ZIP containing manifest.json, transcript.json, optional audio |

## Project Structure

```
src/                    # React frontend
  components/
    common/             # Layout, Sidebar
    editor/             # Waveform, TranscriptView, SegmentEditor, FindReplace, VideoPlayer
    export/             # ExportDialog
    library/            # LibraryList, LibraryFilters, SearchBar, FolderTree
    settings/           # AccelerationSettings
    transcription/      # DropZone, ModelManager, PerformanceBar
  hooks/                # usePlayer (wavesurfer.js audio player hook)
  pages/                # SettingsPage
  stores/               # Zustand stores (settings, transcript, model, recording, library)
  lib/                  # types.ts

src-tauri/              # Rust backend
  src/
    audio/              # Decode + resample
    commands/           # Tauri command handlers (settings, transcription, library, export)
    database/           # SQLite + migrations, search, smart_folders, undo
    export/             # TXT, SRT, VTT renderers + .whisper archive
    models/             # Model manager (download, verify, manage)
    transcription/      # WhisperEngine + pipeline + job management
    settings.rs         # AppSettings with AccelerationBackend
    error.rs            # Typed error enum
  migrations/           # SQL migration files (V001-V009)
  benches/              # Criterion benchmark suite
```

## License

MIT
