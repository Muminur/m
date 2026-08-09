# WhisperDesk

A local-first macOS transcription and translation app built with Tauri 2, React 19, whisper-rs, and CTranslate2. On macOS, Whisper transcription and NLLB translation run entirely on-device when local providers are selected. A Windows desktop build is distributed, but its current binary does not include local Whisper inference; see the platform note below.

## Project Status

**Latest stable release: v1.0.2.** It includes AI actions, advanced batch processing, caption controls, integrations, secure network policy modes, watch-folder improvements, and deep-link support. The one-line installer always installs the latest published release, currently v1.0.2.

### Release highlights (v1.0.2)

Current release adds:

- AI panel for provider/assistant actions plus localized prompts and actions.
- Batch processing dashboard, job status details, and queue lifecycle state.
- Captions mode controls, including source/model selection and auto-display behavior.
- Notion, Obsidian, webhook, and DeepL integrations in the desktop flow.
- Network policy controls to restrict network activity to offline/local/allowed-host modes.
- More reliable watch-folder transcoding with stable completion, cancellation, and fallback behavior.
- Offline translation workflow and YouTube import hardening under strict network and dependency checks.

## Install

### macOS (one-liner)

```bash
curl -fsSL https://raw.githubusercontent.com/Muminur/m/master/scripts/install.sh | bash
```

Downloads the latest release DMG, mounts it, validates and stages `WhisperDesk.app`, and safely replaces the copy in `/Applications`. On first launch, macOS may show a security prompt — go to **System Settings → Privacy & Security → Open Anyway**.

The installer prefers the native build for your Mac. On Apple Silicon, it can fall back to the Intel build through Rosetta when a release does not include an `aarch64` DMG. It never sends or stores GitHub credentials.

### Manual download

Visit [Releases](https://github.com/Muminur/m/releases) and download the DMG for your architecture:

| File                          | Architecture                                           |
| ----------------------------- | ------------------------------------------------------ |
| `WhisperDesk_*_aarch64.dmg`   | Apple Silicon (M1/M2/M3/M4/M5), when available         |
| `WhisperDesk_*_x64.dmg`       | Intel Mac; also works on Apple Silicon through Rosetta |
| `WhisperDesk_*_x64-setup.exe` | Windows 10+                                            |

### Windows

Run `WhisperDesk_*_x64-setup.exe` from the [latest release](https://github.com/Muminur/m/releases/latest).

The current Windows build supports the desktop shell and Windows audio capture, but it does **not** include local Whisper inference or offline NLLB translation. Local file/recording transcription is currently supported on macOS only.

For Gatekeeper help, model setup, source builds, and upgrades, see the [installation guide](docs/INSTALLATION.md).

### Models after installation

Models are downloaded separately and are not bundled with the app:

- On macOS, download a Whisper model from **Models** before local transcription.
- On macOS, for offline translation, open **Settings → Translation**, download **NLLB-200 Distilled 600M (int8)** (about 650 MB), choose a target language, and optionally enable auto-translate.
- Model downloads require network access. Once downloaded, local transcription and NLLB translation run without an API key and without uploading transcript content.

## Features

- **Local file transcription on macOS** — transcribe MP3, WAV, M4A, FLAC, OGG, and OGA files on-device, with native file drop and click-to-select workflows.
- **Model manager** — download, SHA-256 verify, choose a default, and manage Whisper models from tiny through large-v3.
- **Hardware-aware inference** — Apple Silicon can use Metal; Intel Macs use CPU and cannot select an unsupported Metal backend.
- **Recording** — select a microphone, monitor the live level, and start, pause, resume, or stop from the main window.
- **Windows audio capture** — WASAPI system and combined microphone/system recording are available on Windows and disabled on macOS; the current Windows binary cannot locally transcribe those recordings.
- **macOS tray and floating recorder** — record from the menu bar or a draggable control that follows every Space and stays above fullscreen apps.
- **Single-instance state** — a second launch focuses the running app, and recording state remains synchronized across the main and floating webviews.
- **Live transcription feedback** — progress and segments stream into the transcript view; completion, cancellation, and structured errors are correlated to the correct job.
- **Transcript library** — sort transcripts, star or unstar them, move them to Trash, restore them, and permanently delete them. Trashed items are auto-purged after 30 days.
- **Waveform and transcript editing** — play and seek recorded audio, click a segment to seek, edit segment text inline, and use case-aware find and replace.
- **Offline translation on macOS** — download the optional NLLB-200 int8 model, view cached dual subtitles, and optionally translate each completed transcript automatically.
- **Watch folders on macOS** — monitor configured folders and transcribe stable supported audio files in the background with a selected language/model.
- **Optional YouTube import on macOS** — enabled only when compatible `yt-dlp` and `ffmpeg` binaries are installed; unavailable dependencies are explained in the UI.
- **Secure settings** — provider credentials are stored in the system Keychain and are used only when the matching online provider feature is invoked.
- **Updates and automation** — signed-release update checks, `whisperdesk://` deep links, an About dialog, themes, and English/Dutch/German UI strings.

The repository also contains backend modules for exports, AI/cloud actions, folders/tags, and smart folders that are not currently surfaced as primary UI destinations.

## Tech Stack

- **Frontend:** React 19, Tailwind CSS v4, Zustand, react-i18next, Lucide icons, wavesurfer.js
- **Backend:** Tauri 2, Rust, SQLite (rusqlite), whisper-rs 0.16.0
- **Audio:** Symphonia (decode), Rubato (resample), cpal (recording), hound (WAV writing)
- **Inference:** whisper-rs with Metal feature flag (macOS only)
- **Offline translation (macOS):** NLLB-200 Distilled 600M int8 through ct2rs/CTranslate2 with oneDNN + ruy on CPU
- **Backend export modules:** SRT, VTT, TXT, PDF, DOCX, HTML, CSV, JSON, Markdown, and `.whisper` archives
- **Backend integration modules:** Notion, Obsidian, signed webhooks, and DeepL

## Requirements

For release installation:

- macOS 13+ for local Whisper transcription and offline translation
- Windows 10+ for the currently limited Windows desktop/audio-capture build
- Apple Silicon is recommended for Metal-accelerated Whisper transcription; Intel Macs use CPU inference
- About 650 MB of additional disk space if the optional offline translation model is downloaded

For source builds:

- Rust 1.77.2+ and Node.js 22+
- macOS: Xcode Command Line Tools and CMake
- Several gigabytes of free build space; oneDNN is compiled from source on the first translation-enabled build

## Development

```bash
# Install the locked frontend dependencies
npm ci

# Run in development mode (hot reload)
npm run tauri dev

# Build a local macOS app without release-updater signing artifacts
CLANG_RT_DIR="$(xcrun clang -print-runtime-dir)"
RUSTFLAGS="-C link-arg=-L${CLANG_RT_DIR} -C link-arg=-lclang_rt.osx" \
  MACOSX_DEPLOYMENT_TARGET=13.0 \
  npx tauri build --bundles app --config '{"bundle":{"createUpdaterArtifacts":false}}'
```

## Database Migrations

Migrations live in `src-tauri/migrations/` and run automatically on startup:

| Version | Description                                                       |
| ------- | ----------------------------------------------------------------- |
| V001    | Initial schema (transcripts, segments, speakers, models)          |
| V002    | FTS5 full-text search                                             |
| V003    | AI prompt templates                                               |
| V004    | Integrations                                                      |
| V005    | Export presets                                                    |
| V006    | Whisper job tracking                                              |
| V007    | Acceleration stats (backend, realtime factor, wall time)          |
| V008    | Smart folders (id, name, filter_json)                             |
| V009    | FTS index population for existing segments                        |
| V010    | Recordings and watch folder events                                |
| V011    | System audio path for recordings                                  |
| V012    | Dictation history (text, app target, timestamps)                  |
| V013    | Batch jobs and batch job items                                    |
| V014    | Batch job timestamps (started_at, completed_at, processing_ms)    |
| V015    | Batch job model and language settings                             |
| V016    | API keys service registry (actual keys stored in system Keychain) |
| V017    | Cached per-segment transcript translations                        |
| V018    | Offline translation model registry                                |
| V019    | Correct SHA-256 checksums for bundled Whisper model metadata      |
| V020    | Link persisted acceleration statistics to their transcript        |

## Acceleration Backends

| Backend      | Description                     | Status                                                     |
| ------------ | ------------------------------- | ---------------------------------------------------------- |
| Auto         | Use fastest available (default) | Supported                                                  |
| CPU          | Force software inference        | Supported                                                  |
| Metal        | Apple GPU via Metal             | Supported (Apple Silicon only — Intel Macs always use CPU) |
| CoreML + ANE | Apple Neural Engine             | Coming soon                                                |

## Audio Recording

| Source       | Description                       | Platform       |
| ------------ | --------------------------------- | -------------- |
| Microphone   | Input device capture via cpal     | Cross-platform |
| System Audio | WASAPI loopback capture           | Windows only   |
| Combined     | Mic + system audio simultaneously | Windows only   |

## Backend Export Formats

The backend implements these serializers, but the current desktop navigation does not expose the export dialog yet.

| Format   | Description         | Features                                                      |
| -------- | ------------------- | ------------------------------------------------------------- |
| TXT      | Plain text          | Timestamps, speaker labels                                    |
| SRT      | SubRip subtitle     | Millisecond timestamps, speaker tags                          |
| VTT      | WebVTT subtitle     | Millisecond timestamps, speaker tags                          |
| PDF      | Formatted document  | A4/Letter, metadata, page breaks                              |
| DOCX     | Word document       | Styles, speaker headings (OOXML)                              |
| HTML     | Web page            | Interactive timestamps, speaker colors                        |
| CSV      | Spreadsheet         | RFC 4180, per-segment rows                                    |
| JSON     | Structured data     | Metadata, segments, confidence scores                         |
| Markdown | Note format         | Speaker sections, Obsidian/Notion compatible                  |
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
    settings/           # AccelerationSettings, WatchFolderSettings, ApiKeySettings, TranslationSettings
    transcription/      # DropZone, ModelManager, PerformanceBar, TranscriptionSettings
  hooks/                # usePlayer (wavesurfer.js audio player hook)
  i18n/                 # Localization (en.json, nl.json, de.json)
  pages/                # SettingsPage
  stores/               # Zustand stores, including translation and translation-model state
  lib/                  # types.ts, batchTypes.ts, captionTypes.ts, diarizationTypes.ts, aiTypes.ts, trayBridge.ts (tray event ↔ store wiring)
  styles/               # Global CSS (Tailwind)
  test/                 # Component and store tests

src-tauri/              # Rust backend
  src/
    audio/              # Decode, resample, mic recording, system audio, combined capture
    batch/              # Batch processing queue and export
    ai/                 # LLM abstraction: AiProvider trait, ProviderRegistry, 5 providers + OpenAI-compat adapter, actions, templates, cost estimation
    cloud_transcription/ # Cloud transcription: OpenAI Whisper, Deepgram, Groq Whisper, ElevenLabs
    commands/           # Tauri command handlers (settings, transcription, offline/cloud translation, library, export, recording, watch, dictation, shortcuts, batch, diarization, import, AI, keychain)
    database/           # SQLite + migrations, search, smart_folders, recordings, undo
    dictation/          # Dictation pipeline: accessibility, postprocessing, AI correction, history
    diarization/        # Speaker diarization: tinydiarize, ElevenLabs, Deepgram providers
    export/             # TXT, SRT, VTT renderers + .whisper archive
    import/             # YouTube import via yt-dlp, yt-dlp detection
    integrations/       # Notion, Obsidian, webhooks, DeepL
    models/             # Model manager (download, verify, manage)
    shortcuts/          # Global shortcut manager with collision detection
    transcription/      # WhisperEngine + pipeline + streaming + VAD + translation + filler word removal + hybrid cloud refinement
    translation/        # Offline NLLB engine, language mapping, model paths, and engine lifecycle
    watch/              # Watch folder manager + audio file handler
    network/            # NetworkGuard module (HTTP policy enforcement)
    settings.rs         # AppSettings with AccelerationBackend and NetworkPolicy
    tray.rs             # macOS menu bar tray (state-aware icon, menu actions handled in Rust to bypass webview suspension)
    error.rs            # Typed error enum (14 error categories with codes)
    keychain.rs         # macOS Keychain integration for API key storage
    logging.rs          # Tracing/logging infrastructure with file rotation
  migrations/           # SQL migration files (V001-V020)
  benches/              # Criterion benchmark suite

scripts/
  install.sh            # One-liner macOS installer (downloads latest DMG from GitHub Releases)
  reinstall.sh          # Rebuild from source and reinstall to /Applications
  clean-build-verify.sh # Full clean build with TypeScript, Clippy, and feature checks

workers/
  updater/              # Cloudflare Worker serving Tauri auto-update manifests
```

## Scripts

| Script                             | Purpose                                                                                     |
| ---------------------------------- | ------------------------------------------------------------------------------------------- |
| `scripts/install.sh`               | Detect Mac architecture, download the latest compatible DMG, and install to `/Applications` |
| `scripts/reinstall.sh`             | Build from source and reinstall (accepts `--keep-data`, `--skip-build`, `--launch`)         |
| `scripts/clean-build-verify.sh`    | Full clean build with TypeScript, Clippy, and feature verification                          |
| `scripts/generate-updater-keys.sh` | Generate Tauri updater signing key pair                                                     |

## CI / CD

- **CI** (`.github/workflows/ci.yml`): Rust checks (fmt, clippy, tests) + frontend checks (tsc, vitest, eslint) on every push/PR
- **Release** (`.github/workflows/release.yml`): Builds and publishes a GitHub Release on every `v*` tag push
  - macOS Apple Silicon: `WhisperDesk_*_aarch64.dmg`
  - macOS Intel: `WhisperDesk_*_x64.dmg`
  - Windows x64: `WhisperDesk_*_x64-setup.exe` + `WhisperDesk_*_x64_en-US.msi`
- **Auto-update**: Cloudflare Worker at `whisperdesk-updater.whisperdesk.workers.dev` proxies GitHub Releases to serve Tauri-compatible update manifests

## License

MIT
