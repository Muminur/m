# Installing WhisperDesk

The latest published release is v1.0.1. Release installers contain the application only; Whisper transcription models and the optional offline translation model are downloaded from inside WhisperDesk.

## macOS one-line installer

```bash
curl -fsSL https://raw.githubusercontent.com/Muminur/m/master/scripts/install.sh | bash
```

This command follows the installer on `master` and installs the latest published GitHub Release, not an unreleased feature branch. The script:

1. Detects Apple Silicon (`arm64`) or Intel (`x86_64`).
2. Reads the public GitHub Releases API without a token or other credentials.
3. Downloads the matching DMG over HTTPS.
4. Mounts it, copies `WhisperDesk.app` to `/Applications`, and cleans up its temporary files.

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

On macOS, open the DMG and copy WhisperDesk to Applications. If Gatekeeper blocks the first launch, open **System Settings → Privacy & Security** and choose **Open Anyway** for WhisperDesk.

## Downloading models

No speech or translation model is bundled in the installer.

### Local transcription

1. Open **Models**.
2. Download a Whisper model. Smaller models use less disk and run faster; larger models are generally more accurate.
3. Optionally choose the default model in **Settings** for tray and automatic transcription.

### Offline translation on macOS

1. Open **Settings → Translation**.
2. Download **NLLB-200 Distilled 600M (int8)**, which uses about 650 MB.
3. Choose Bengali (Bangla), Arabic, or English as the target.
4. Optionally enable **Auto-translate after transcription**.

The model download requires external network access. If WhisperDesk is in Offline or Local Only mode, temporarily select **Allow All** for the download. Afterward, NLLB translation runs locally on CPU and does not require an API key.

## Updating or reinstalling

Running the one-line installer again replaces only `/Applications/WhisperDesk.app`; application data and downloaded models are preserved.

For a source checkout, preserve the database and models with:

```bash
./scripts/reinstall.sh --keep-data --launch
```

Warning: `scripts/reinstall.sh` without `--keep-data` intentionally deletes WhisperDesk's Application Support data, caches, database, settings, and downloaded models before reinstalling.

## Building from source

WhisperDesk source builds require Rust 1.77+, Node.js 22+, Git, and the current [Tauri 2 prerequisites](https://v2.tauri.app/start/prerequisites/).

For a macOS desktop build, install Xcode Command Line Tools and CMake first:

```bash
xcode-select --install
brew install cmake
```

Then build from a checkout:

```bash
npm ci
npm run tauri build
```

The first macOS build compiles oneDNN for offline NLLB translation. It can take substantially longer than later builds and needs several gigabytes of free temporary build space. Release installation does not require Rust, Node.js, Xcode, or CMake.

On Windows, install Microsoft C++ Build Tools and WebView2 as described in the Tauri prerequisites before building. Offline NLLB translation is currently available only in the macOS build; other WhisperDesk features remain cross-platform as documented in the main README.
