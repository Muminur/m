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
- **Export** — TXT, SRT, VTT, PDF, DOCX, HTML, CSV, JSON, and Markdown formats with speaker labels and timestamps
- **PDF export** — formatted A4/Letter PDF with header, metadata, speaker-labeled segments, and automatic page breaks
- **DOCX export** — Word document generated via OOXML templates (zip+handlebars); opens correctly in Word/Pages with styles and speaker headings
- **HTML export** — styled HTML with interactive timestamp anchors and speaker color coding
- **CSV export** — RFC 4180 compliant with columns: start_ms, end_ms, timestamps, speaker, text
- **JSON export** — structured JSON with metadata and per-segment data including confidence scores
- **Markdown export** — speaker-grouped sections with timestamps for Obsidian, Notion, and note-taking apps
- **Custom export templates** — write Handlebars templates with `{{segments}}`, `{{title}}`, `{{duration}}` variables for fully custom output formats
- **WhisperDesk archive** — `.whisper` ZIP+JSON format for lossless transcript export/import with audio
- **Copy to clipboard** — copy full transcript or selected segments
- **Video player** — inline video playback with synced subtitle overlay for video source files
- **Audio recording** — microphone capture via cpal with real-time VU meter and pause/resume
- **System audio capture** — WASAPI loopback recording on Windows for system audio
- **Combined recording** — simultaneous mic + system audio capture with mixed output
- **Device selector** — choose input device from available audio hardware
- **Watch folders** — auto-transcribe new audio files dropped into configured folders
- **Real-time streaming** — segments appear as they are transcribed
- **Export dialog** — format picker, option toggles, destination picker with preview
- **Streaming transcription** — sliding window real-time transcription (3s step, 10s context, 200ms overlap)
- **Voice activity detection** — Silero VAD integration; silence produces no hallucinated text
- **Floating captions** — always-on-top translucent overlay with 1-3 rolling lines, configurable font/color/opacity
- **System audio captions** — real-time captioning of system audio playback
- **Dictation mode** — double-tap Right Command to dictate; text inserted into active app via Accessibility API
- **Punctuation commands** — say "period", "comma", "new line" etc. with auto-capitalization
- **AI-enhanced dictation** — optional grammar/spelling correction via configurable AI provider
- **Dictation history** — last 50 dictated snippets in menubar; click to re-insert
- **Spotlight bar** — Cmd+Shift+Space global input bar: speak, see text, copy or insert
- **Live translation** — real-time caption translation via Whisper translate mode or DeepL API
- **Global shortcuts** — configurable hotkeys with collision detection and conflict resolution
- **Speaker diarization** — local tinydiarize speaker turn detection; cloud diarization via ElevenLabs Scribe and Deepgram Nova
- **Speaker labels** — per-speaker colors, inline rename; speaker count hint before transcription
- **Batch processing** — queue multiple files with configurable concurrency (1-4); per-file progress, pause/resume/cancel
- **Batch export** — export all completed batch items to TXT/SRT/VTT in one operation
- **YouTube import** — paste YouTube URL; audio extracted via yt-dlp and queued for transcription
- **yt-dlp detection** — auto-detect yt-dlp in PATH, Homebrew, or local app data
- **Filler word removal** — configurable word list (um, uh, er, like, you know); word-boundary aware
- **Privacy-first architecture** — NetworkGuard enforces offline/local-only/allow-all network policies; all HTTP routed through a single guard module
- **AI Action Panel** — summarize, extract key points, Q&A, translate, rewrite, or generate chapters from any transcript using 9 LLM providers
- **LLM providers** — OpenAI (GPT-4o, GPT-4o-mini), Anthropic (Claude Opus/Sonnet/Haiku), Groq (llama3, mixtral), Ollama (local), DeepSeek, xAI, OpenRouter, Azure, and custom OpenAI-compatible endpoints
- **Streaming AI responses** — real-time token streaming from AI providers with animated cursor display
- **Token and cost estimation** — estimated token count and USD cost shown before sending to any paid API
- **Prompt templates** — create, edit, and reuse custom AI prompts with `{{transcript}}`, `{{speaker_list}}`, and `{{duration}}` variable substitution
- **Cloud transcription** — upload audio to OpenAI Whisper, Deepgram Nova-2 (with diarization), Groq Whisper, or ElevenLabs with explicit opt-in and cost estimate shown first
- **Hybrid transcription** — transcribe locally then refine with cloud in one click
- **API key management** — all provider keys stored in macOS Keychain (never written to disk); manage from Settings
- **Notion integration** — push transcripts to any Notion database via the Notion API; configurable database ID; returns page URL
- **Obsidian integration** — write transcripts as `.md` files to any Obsidian vault folder with YAML frontmatter (date, duration, language, speakers)
- **Webhook system** — POST transcript JSON to any Zapier, Make, n8n, or custom endpoint on transcription complete; HMAC-SHA256 request signing; SSRF-protected URL validation
- **DeepL translation** — translate full transcripts or individual subtitle segments to 30+ languages; auto-detects free vs Pro API endpoint; preserves SRT/VTT structure
- **Dual subtitles** — display original and DeepL-translated subtitles side-by-side with active segment highlighting synchronized to video playback
- **Integration wizard** — step-by-step setup UI for all integrations: API key entry, vault/database configuration, connection testing
- **Apple Shortcuts** — "Transcribe File", "Get Transcript", "Start/Stop Recording" intent stubs for automation workflows (full implementation in M10)
- **macOS Share Sheet** — share transcripts via AirDrop, Mail, Messages via NSSharingService (full Swift plugin in M10)
- **Localization** — i18n support via react-i18next with English, Dutch, and German translations
- **Typed error handling** — all backend commands return typed `AppError` variants with error codes; no raw string errors (14 error categories)

## Tech Stack

- **Frontend:** React 19, Tailwind CSS v4, Zustand, react-i18next, Lucide icons, wavesurfer.js
- **Backend:** Tauri 2, Rust, SQLite (rusqlite), whisper-rs 0.16.0
- **Audio:** Symphonia (decode), Rubato (resample), cpal (recording), hound (WAV writing)
- **Inference:** whisper-rs with Metal feature flag (macOS only)
- **Export:** SRT, VTT, TXT, PDF (printpdf), DOCX (zip+handlebars OOXML), HTML, CSV, JSON, Markdown, ZIP-based .whisper archive
- **Integrations:** Notion API, Obsidian vault, webhooks (HMAC-SHA256 signed), DeepL translation API

## Requirements

- macOS 12+ (Apple Silicon recommended for Metal acceleration) or Windows 10+
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
| V010 | Recordings and watch folder events |
| V011 | System audio path for recordings |
| V012 | Dictation history (text, app target, timestamps) |
| V013 | Batch jobs and batch job items |
| V014 | Batch job timestamps (started_at, completed_at, processing_ms) |
| V015 | Batch job model and language settings |
| V016 | API keys service registry (actual keys stored in system Keychain) |

## Acceleration Backends

| Backend | Description | Status |
|---------|-------------|--------|
| Auto | Use fastest available (default) | Supported |
| CPU | Force software inference | Supported |
| Metal | Apple GPU via Metal | Supported (macOS only) |
| CoreML + ANE | Apple Neural Engine | Coming soon |

## Audio Recording

| Source | Description | Platform |
|--------|-------------|----------|
| Microphone | Input device capture via cpal | Cross-platform |
| System Audio | WASAPI loopback capture | Windows only |
| Combined | Mic + system audio simultaneously | Windows only |

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
    ai/                 # AiPanel, ProviderSelector (streaming AI response panel)
    common/             # Layout, Sidebar
    editor/             # Waveform, TranscriptView, SegmentEditor, FindReplace, VideoPlayer, SpeakerLabels
    export/             # ExportDialog
    library/            # LibraryList, LibraryFilters, SearchBar, FolderTree, TranscriptDetail
    batch/              # BatchDashboard
    captions/           # CaptionOverlay, CaptionControls, SpotlightBar
    recording/          # RecordingPanel, DeviceSelector, SpeakerCountHint, CloudTranscription
    settings/           # AccelerationSettings, WatchFolderSettings, ApiKeySettings
    transcription/      # DropZone, ModelManager, PerformanceBar, TranscriptionSettings
  hooks/                # usePlayer (wavesurfer.js audio player hook)
  i18n/                 # Localization (en.json, nl.json, de.json)
  pages/                # SettingsPage
  stores/               # Zustand stores (settings, transcript, model, recording, library, caption, batch, ai)
  lib/                  # types.ts, batchTypes.ts, captionTypes.ts, diarizationTypes.ts, aiTypes.ts
  styles/               # Global CSS (Tailwind)
  test/                 # Component and store tests

src-tauri/              # Rust backend
  src/
    audio/              # Decode, resample, mic recording, system audio, combined capture
    batch/              # Batch processing queue and export
    ai/                 # LLM abstraction: AiProvider trait, ProviderRegistry, 5 providers + OpenAI-compat adapter, actions, templates, cost estimation
    cloud_transcription/ # Cloud transcription: OpenAI Whisper, Deepgram, Groq Whisper, ElevenLabs, VibeVoice
    commands/           # Tauri command handlers (settings, transcription, library, export, recording, watch, dictation, translate, shortcuts, batch, diarization, import, ai, keychain, cloud_transcription)
    database/           # SQLite + migrations, search, smart_folders, recordings, undo
    dictation/          # Dictation pipeline: accessibility, postprocessing, AI correction, history
    diarization/        # Speaker diarization: tinydiarize, ElevenLabs, Deepgram providers
    export/             # TXT, SRT, VTT renderers + .whisper archive
    import/             # YouTube import via yt-dlp, yt-dlp detection
    models/             # Model manager (download, verify, manage)
    shortcuts/          # Global shortcut manager with collision detection
    transcription/      # WhisperEngine + pipeline + streaming + VAD + translation + filler word removal + hybrid cloud refinement
    watch/              # Watch folder manager + audio file handler
    network/            # NetworkGuard module (HTTP policy enforcement)
    settings.rs         # AppSettings with AccelerationBackend and NetworkPolicy
    error.rs            # Typed error enum (14 error categories with codes)
    keychain.rs         # macOS Keychain integration for API key storage
    logging.rs          # Tracing/logging infrastructure with file rotation
  migrations/           # SQL migration files (V001-V016)
  benches/              # Criterion benchmark suite
```

## License

MIT
