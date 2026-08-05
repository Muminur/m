export interface TranscriptionParams {
  language: string | null;
  translate: boolean;
  beamSize: number;
  temperature: number;
  nThreads: number;
  wordTimestamps: boolean;
  initialPrompt: string | null;
  noSpeechThreshold: number | null;
}

export const DEFAULT_PARAMS: TranscriptionParams = {
  language: null,
  translate: false,
  beamSize: 5,
  temperature: 0,
  nThreads: 4,
  wordTimestamps: false,
  initialPrompt: null,
  noSpeechThreshold: null,
};
