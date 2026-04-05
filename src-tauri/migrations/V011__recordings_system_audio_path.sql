-- Add system_audio_path column for combined recordings
ALTER TABLE recordings ADD COLUMN system_audio_path TEXT;
