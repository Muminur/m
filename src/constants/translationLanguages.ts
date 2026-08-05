// FLORES-200 codes understood by NLLB. value = FLORES code, label = human name.
export const TRANSLATION_LANGUAGES: { value: string; label: string }[] = [
  { value: "ben_Beng", label: "Bengali (Bangla)" },
  { value: "arb_Arab", label: "Arabic" },
  { value: "eng_Latn", label: "English" },
];

// The offline NLLB translation model id (matches the Rust NLLB_MODEL_ID).
export const NLLB_MODEL_ID = "nllb-200-distilled-600M-int8";

// Whisper ISO 639-1 source codes → FLORES-200 codes, for the supported target
// languages. Used only to skip auto-translation when the detected source
// language already equals the target (e.g. English → English).
export const ISO_TO_FLORES: Record<string, string> = {
  en: "eng_Latn",
  bn: "ben_Beng",
  ar: "arb_Arab",
};
