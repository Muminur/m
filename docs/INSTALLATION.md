# Installing WhisperDesk

The latest published release is v1.0.1. Release installers contain the application only; Whisper transcription models and the optional offline translation model are downloaded from inside WhisperDesk.

The current `master` branch is newer than v1.0.1. It includes offline translation, the macOS floating recorder, direct **Transcribe File** navigation, and reliability/platform fixes that will not be installed by the one-line command until the next release is tagged. To run those changes now, use [Building the current source](#building-the-current-source).

## macOS one-line installer

```bash
curl -fsSL https://raw.githubusercontent.com/Muminur/m/master/scripts/install.sh | bash
```

This command downloads the installer script from `master`, but the script installs the latest published GitHub Release rather than an unreleased source revision. It requires macOS 13 or newer and:

1. Detects Apple Silicon (`arm64`) or Intel (`x86_64`).
2. Resolves GitHub's public latest-release redirect without an API token or other credentials, so it does not depend on the unauthenticated API rate limit.
3. Downloads the matching DMG over HTTPS.
4. Mounts the DMG and validates the expected app name and bundle identifier.
5. Stages the new app before replacing `/Applications/WhisperDesk.app`, restoring the previous copy if the final swap fails.
6. Cleans up the mounted image and permission-restricted temporary files.

On Apple Silicon, the installer prefers an `aarch64` DMG. If a release only has an `x64` DMG, it installs that build with a Rosetta warning. An Apple Silicon DMG is never offered to an Intel Mac.

To inspect the installer before running it:

```bash
curl -fsSL https://raw.githubusercontent.com/Muminur/m/master/scripts/install.sh
```

## Manual installation

Open the [latest GitHub Release](https://github.com/Muminur/m/releases/latest) and select the appropriate file:

| Asset                         | Use on                                      |
| ----------------------------- | ------------------------------------------- |
| `WhisperDesk_*_aarch64.dmg`   | Apple Silicon, when the release provides it |
| `WhisperDesk_*_x64.dmg`       | Intel Mac, or Apple Silicon through Rosetta |
| `WhisperDesk_*_x64-setup.exe` | Windows 10+                                 |
| `WhisperDesk_*_x64_en-US.msi` | Windows 10+ administrative/MSI deployment   |

The Windows package currently provides the desktop shell and Windows audio capture, but it does not include local Whisper inference or offline NLLB translation. Local transcription is supported on macOS only in this version.

On macOS, open the DMG and copy WhisperDesk to Applications. If Gatekeeper blocks the first launch, open **System Settings → Privacy & Security** and choose **Open Anyway** for WhisperDesk.

## First launch and platform choices

- Allow **Microphone** access when prompted if you want to record audio.
- On macOS, recording supports the microphone. **System Audio** and **Both** are Windows-only choices and are disabled rather than silently falling back to the microphone.
- Apple Silicon Macs can use Metal acceleration. Intel Macs use CPU inference, and the unavailable Metal setting is disabled.
- In current `master`, choose **Transcribe File** in the sidebar to process an existing audio file.
- In current `master`, the floating recorder appears as a draggable pill on first launch, remains visible across Spaces and fullscreen apps, and does not steal focus. Toggle it from **Floating Recorder** in the menu bar tray. Its visibility choice is remembered.

## Downloading models

No speech or translation model is bundled in the installer.

### Local transcription on macOS

1. Open **Models**.
2. Download a Whisper model. Smaller models use less disk and run faster; larger models are generally more accurate.
3. Optionally choose the default model in **Settings** for tray and automatic transcription.

Downloading a Whisper model in the current Windows build does not enable local inference; the Windows transcription engine is not compiled into this release.

### Offline translation on macOS

1. Open **Settings → Translation**.
2. Download **NLLB-200 Distilled 600M (int8)**, which uses about 650 MB.
3. Choose Bengali (Bangla), Arabic, or English as the target.
4. Optionally enable **Auto-translate after transcription**.

The model download requires external network access. Afterward, NLLB translation runs locally on CPU and does not require an API key.

## Optional YouTube import dependencies

The **Transcribe File → Import from YouTube** control is disabled until both `yt-dlp` and `ffmpeg` are installed. On macOS with Homebrew:

```bash
brew install yt-dlp ffmpeg
```

WhisperDesk also detects compatible binaries available on `PATH` and common Homebrew locations. These tools are optional and are not installed by the one-line app installer.

## Updating or reinstalling

Running the one-line installer again replaces only `/Applications/WhisperDesk.app`; application data and downloaded models are preserved.

The v1.0.1 macOS release does not include the signed updater archive required for an in-app update. Re-run the one-line installer or install the next DMG manually. Future signed releases can use **Settings → Updates**.

For a source checkout, build first as described below, then preserve the database and models while installing that completed build:

```bash
./scripts/reinstall.sh --keep-data --skip-build --launch
```

Warning: `scripts/reinstall.sh` without `--keep-data` intentionally deletes WhisperDesk's Application Support data, caches, database, settings, and downloaded models before reinstalling.

## Building the current source

WhisperDesk source builds require Rust 1.77.2+, Node.js 22+, Git, and the current [Tauri 2 prerequisites](https://v2.tauri.app/start/prerequisites/).

For a macOS desktop build, install Xcode Command Line Tools and CMake first:

```bash
xcode-select --install
brew install cmake
```

Then build a local macOS app from a checkout. The Clang runtime link flags support the Metal dependency on macOS 13, while the config override avoids generating release-updater artifacts that require the private signing key used only by release automation:

```bash
npm ci
CLANG_RT_DIR="$(xcrun clang -print-runtime-dir)"
RUSTFLAGS="-C link-arg=-L${CLANG_RT_DIR} -C link-arg=-lclang_rt.osx" \
  MACOSX_DEPLOYMENT_TARGET=13.0 \
  npx tauri build --bundles app --config '{"bundle":{"createUpdaterArtifacts":false}}'
./scripts/reinstall.sh --keep-data --skip-build --launch
```

The first macOS build compiles oneDNN for offline NLLB translation. It can take substantially longer than later builds and needs several gigabytes of free temporary build space. Do not commit or publish updater private keys, API keys, Keychain exports, `.env` files, or other credentials. Release installation does not require Rust, Node.js, Xcode, or CMake.

On Windows, install Microsoft C++ Build Tools and WebView2 as described in the Tauri prerequisites before building. The current source still excludes whisper-rs and NLLB from Windows, so a Windows source build has the same local transcription/translation limitation as the release package.
