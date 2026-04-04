/// Transcription benchmark suite — measures realtime factor for CPU and Metal backends.
///
/// Requires a model file at the path set by WHISPER_BENCH_MODEL_PATH env var
/// (defaults to checking for tiny.en in the user's app data models dir).
/// Skips gracefully if no model is found.
///
/// Run with:
///   cargo bench --bench transcription_bench
///
/// Realtime factor = audio_duration / wall_time. Higher = faster than realtime.
use criterion::{criterion_group, criterion_main, Criterion};

#[cfg(target_os = "macos")]
mod macos_benches {
    use std::path::PathBuf;
    use criterion::Criterion;

    /// Locate a whisper model file for benchmarking.
    /// Checks WHISPER_BENCH_MODEL_PATH env var, then common locations.
    fn find_model() -> Option<PathBuf> {
        // Explicit override via env var
        if let Ok(path) = std::env::var("WHISPER_BENCH_MODEL_PATH") {
            let p = PathBuf::from(path);
            if p.exists() {
                return Some(p);
            }
        }
        // Common locations: project fixtures dir
        let candidates = [
            PathBuf::from("benches/fixtures/ggml-tiny.en.bin"),
            PathBuf::from("benches/fixtures/ggml-base.en.bin"),
        ];
        for c in &candidates {
            if c.exists() {
                return Some(c.clone());
            }
        }
        None
    }

    /// 10-second silent PCM at 16 kHz mono f32 (for compile-time bench validation).
    /// Real benchmarks should use an actual speech fixture for meaningful realtime factors.
    fn silent_pcm_10s() -> Vec<f32> {
        vec![0.0f32; 16_000 * 10]
    }

    pub fn bench_cpu(c: &mut Criterion) {
        let model_path = match find_model() {
            Some(p) => p,
            None => {
                eprintln!("transcription_bench: skipping CPU bench — no model found");
                eprintln!("  Set WHISPER_BENCH_MODEL_PATH or place a model in benches/fixtures/");
                return;
            }
        };

        use whisper_desk_app_lib::transcription::engine::{WhisperEngine, TranscriptionParams};
        use whisper_desk_app_lib::settings::AccelerationBackend;
        use std::sync::{Arc, atomic::AtomicBool};

        let engine = match WhisperEngine::new(&model_path, AccelerationBackend::Cpu) {
            Ok(e) => e,
            Err(e) => {
                eprintln!("transcription_bench: failed to load model: {}", e);
                return;
            }
        };

        let pcm = silent_pcm_10s();
        let params = TranscriptionParams::default();

        c.bench_function("transcription_cpu_tiny_10s", |b| {
            b.iter(|| {
                let abort = Arc::new(AtomicBool::new(false));
                let _ = engine.transcribe(&params, &pcm, |_| {}, Arc::clone(&abort));
            });
        });
    }

    pub fn bench_metal(c: &mut Criterion) {
        let model_path = match find_model() {
            Some(p) => p,
            None => {
                eprintln!("transcription_bench: skipping Metal bench — no model found");
                return;
            }
        };

        use whisper_desk_app_lib::transcription::engine::{WhisperEngine, TranscriptionParams};
        use whisper_desk_app_lib::settings::AccelerationBackend;
        use std::sync::{Arc, atomic::AtomicBool};

        let engine = match WhisperEngine::new(&model_path, AccelerationBackend::Metal) {
            Ok(e) => e,
            Err(e) => {
                eprintln!("transcription_bench: failed to load model for Metal: {}", e);
                return;
            }
        };

        let pcm = silent_pcm_10s();
        let params = TranscriptionParams::default();

        c.bench_function("transcription_metal_tiny_10s", |b| {
            b.iter(|| {
                let abort = Arc::new(AtomicBool::new(false));
                let _ = engine.transcribe(&params, &pcm, |_| {}, Arc::clone(&abort));
            });
        });
    }
}

#[cfg(target_os = "macos")]
criterion_group!(benches, macos_benches::bench_cpu, macos_benches::bench_metal);

#[cfg(not(target_os = "macos"))]
fn noop_bench(_c: &mut Criterion) {
    eprintln!("transcription_bench: skipping — macOS only");
}

#[cfg(not(target_os = "macos"))]
criterion_group!(benches, noop_bench);

criterion_main!(benches);
