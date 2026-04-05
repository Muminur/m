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
    use criterion::Criterion;
    use std::path::PathBuf;
    use std::sync::{atomic::AtomicBool, Arc};
    use whisper_desk_app_lib::settings::AccelerationBackend;
    use whisper_desk_app_lib::transcription::engine::{TranscriptionParams, WhisperEngine};

    fn find_model() -> Option<PathBuf> {
        if let Ok(path) = std::env::var("WHISPER_BENCH_MODEL_PATH") {
            let p = PathBuf::from(path);
            if p.exists() {
                return Some(p);
            }
        }
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

    fn silent_pcm_10s() -> Vec<f32> {
        vec![0.0f32; 16_000 * 10]
    }

    fn bench_backend(c: &mut Criterion, backend: AccelerationBackend, name: &str) {
        let model_path = match find_model() {
            Some(p) => p,
            None => {
                eprintln!(
                    "transcription_bench: skipping {} bench — no model found",
                    name
                );
                eprintln!("  Set WHISPER_BENCH_MODEL_PATH or place a model in benches/fixtures/");
                return;
            }
        };
        let engine = match WhisperEngine::new(&model_path, backend) {
            Ok(e) => e,
            Err(e) => {
                eprintln!(
                    "transcription_bench: failed to load model for {}: {}",
                    name, e
                );
                return;
            }
        };
        let pcm = silent_pcm_10s();
        let params = TranscriptionParams::default();
        c.bench_function(&format!("transcription_{}_tiny_10s", name), |b| {
            b.iter(|| {
                let abort = Arc::new(AtomicBool::new(false));
                let _ = engine.transcribe(&params, &pcm, |_| {}, Arc::clone(&abort));
            });
        });
    }

    pub fn bench_cpu(c: &mut Criterion) {
        bench_backend(c, AccelerationBackend::Cpu, "cpu");
    }

    pub fn bench_metal(c: &mut Criterion) {
        bench_backend(c, AccelerationBackend::Metal, "metal");
    }
}

#[cfg(target_os = "macos")]
criterion_group!(
    benches,
    macos_benches::bench_cpu,
    macos_benches::bench_metal
);

#[cfg(not(target_os = "macos"))]
fn noop_bench(_c: &mut Criterion) {
    eprintln!("transcription_bench: skipping — macOS only");
}

#[cfg(not(target_os = "macos"))]
criterion_group!(benches, noop_bench);

criterion_main!(benches);
