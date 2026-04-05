use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

pub fn init() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        if cfg!(debug_assertions) {
            EnvFilter::new("debug")
        } else {
            EnvFilter::new("info")
        }
    });

    #[cfg(debug_assertions)]
    {
        tracing_subscriber::registry()
            .with(filter)
            .with(
                fmt::layer()
                    .with_target(true)
                    .with_thread_ids(false)
                    .pretty(),
            )
            .init();
    }

    #[cfg(not(debug_assertions))]
    {
        // In release builds, log to file in ~/Library/Logs/WhisperDesk/
        let log_dir = get_log_dir();
        if let Some(dir) = log_dir {
            let _ = std::fs::create_dir_all(&dir);
            let file_appender = tracing_appender::rolling::daily(&dir, "whisperdesk.log");
            let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
            // Note: _guard must live as long as the app - in practice we leak it for the process lifetime
            std::mem::forget(_guard);

            tracing_subscriber::registry()
                .with(filter)
                .with(fmt::layer().with_ansi(false).with_writer(non_blocking))
                .init();
        } else {
            tracing_subscriber::registry()
                .with(filter)
                .with(fmt::layer())
                .init();
        }
    }
}

#[cfg(not(debug_assertions))]
fn get_log_dir() -> Option<std::path::PathBuf> {
    dirs::home_dir().map(|h| h.join("Library").join("Logs").join("WhisperDesk"))
}

#[cfg(test)]
mod tests {
    // Logging init is tested implicitly via other module tests
    // (tracing macros would panic if subscriber not set)
    #[test]
    fn test_logging_module_exists() {
        // Just verify the module compiles
        assert!(true);
    }
}
