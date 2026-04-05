use crate::audio::mic::{self, MicRecorder};
use crate::error::AppError;
use std::path::PathBuf;

/// Combined audio capture: microphone + system audio mixed together.
///
/// On Windows: uses cpal mic input + WASAPI loopback output
/// On macOS: uses cpal mic input + ScreenCaptureKit (future)
///
/// When system audio is unavailable, falls back to mic-only capture.

pub struct CombinedCapture {
    mic: MicRecorder,
    #[cfg(target_os = "windows")]
    system: Option<super::system_audio::wasapi_loopback::SystemAudioCapture>,
    _mic_path: PathBuf,
    _system_path: Option<PathBuf>,
}

impl CombinedCapture {
    pub fn new(
        device_id: Option<&str>,
        mic_output: PathBuf,
        system_output: PathBuf,
    ) -> Result<Self, AppError> {
        let device = mic::get_device_by_id(device_id)?;
        let mic = MicRecorder::new(&device, mic_output.clone())?;

        #[cfg(target_os = "windows")]
        let system = {
            match super::system_audio::wasapi_loopback::SystemAudioCapture::new(
                system_output.clone(),
            ) {
                Ok(sys) => Some(sys),
                Err(e) => {
                    tracing::warn!("System audio capture unavailable, mic-only mode: {}", e);
                    None
                }
            }
        };

        Ok(Self {
            mic,
            #[cfg(target_os = "windows")]
            system,
            _mic_path: mic_output,
            _system_path: Some(system_output),
        })
    }

    pub fn start(&self) -> Result<(), AppError> {
        self.mic.start()?;

        #[cfg(target_os = "windows")]
        if let Some(ref sys) = self.system {
            sys.start()?;
        }

        Ok(())
    }

    pub fn pause(&self) {
        self.mic.pause();
        #[cfg(target_os = "windows")]
        if let Some(ref sys) = self.system {
            sys.pause();
        }
    }

    pub fn resume(&self) {
        self.mic.resume();
        #[cfg(target_os = "windows")]
        if let Some(ref sys) = self.system {
            sys.resume();
        }
    }

    pub fn get_mic_level_db(&self) -> f32 {
        self.mic.get_level_db()
    }

    pub fn duration_ms(&self) -> u64 {
        self.mic.duration_ms()
    }

    pub fn mic_sample_rate(&self) -> u32 {
        self.mic.sample_rate()
    }

    pub fn mic_channels(&self) -> u16 {
        self.mic.channels()
    }

    pub fn stop(self) -> Result<(PathBuf, Option<PathBuf>), AppError> {
        let mic_path = self.mic.stop()?;

        #[cfg(target_os = "windows")]
        let sys_path = if let Some(sys) = self.system {
            Some(sys.stop()?)
        } else {
            None
        };

        #[cfg(not(target_os = "windows"))]
        let sys_path: Option<PathBuf> = None;

        Ok((mic_path, sys_path))
    }
}
