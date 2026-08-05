import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useSettingsStore } from "@/stores/settingsStore";
import type { AccelerationBackend } from "@/lib/types";
import { formatError } from "@/lib/formatError";

interface Option {
  value: AccelerationBackend;
  label: string;
  description: string;
  disabled?: boolean;
  disabledReason?: string;
}

const OPTIONS: Option[] = [
  {
    value: "auto",
    label: "Auto",
    description: "Use the fastest available backend (recommended)",
  },
  {
    value: "cpu",
    label: "CPU Only",
    description: "Software inference — no GPU required",
  },
  {
    value: "metal",
    label: "Metal (GPU)",
    description: "Apple GPU acceleration via Metal",
  },
  {
    value: "core_ml",
    label: "CoreML + ANE",
    description: "Apple Neural Engine — fastest on Apple Silicon",
    disabled: true,
    disabledReason: "Coming soon — requires model packages not yet available",
  },
];

export function AccelerationSettings() {
  const { settings, updateSettings } = useSettingsStore();
  const current = settings?.accelerationBackend ?? "auto";
  const [metalAvailable, setMetalAvailable] = useState<boolean | null>(null);

  useEffect(() => {
    let cancelled = false;
    invoke<boolean>("is_metal_available")
      .then((available) => {
        if (!cancelled) setMetalAvailable(available);
      })
      .catch(() => {
        if (!cancelled) setMetalAvailable(false);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const options = OPTIONS.map((option) => {
    if (option.value !== "metal") return option;
    return {
      ...option,
      disabled: metalAvailable !== true,
      disabledReason:
        metalAvailable === null
          ? "Checking hardware support..."
          : "Unavailable — Metal requires an Apple Silicon Mac",
    };
  });

  return (
    <div className="space-y-3">
      <div>
        <h3 className="text-sm font-medium">Acceleration Backend</h3>
        <p className="text-xs text-muted-foreground mt-0.5">
          Controls how transcription inference is executed. Auto is recommended.
        </p>
      </div>

      <div className="space-y-2">
        {options.map((opt) => (
          <label
            key={opt.value}
            className={`flex items-start gap-3 p-3 rounded-md border cursor-pointer transition-colors ${
              opt.disabled
                ? "opacity-50 cursor-not-allowed border-border"
                : current === opt.value
                  ? "border-primary bg-primary/5"
                  : "border-border hover:border-primary/50 hover:bg-accent/50"
            }`}
          >
            <input
              type="radio"
              name="acceleration_backend"
              value={opt.value}
              checked={current === opt.value}
              disabled={opt.disabled}
              onChange={() => {
                void updateSettings({ accelerationBackend: opt.value }).catch((error) => {
                  console.error("Failed to update acceleration backend:", formatError(error));
                });
              }}
              className="mt-0.5 flex-none"
            />
            <div className="min-w-0">
              <span className="text-sm font-medium">{opt.label}</span>
              <p className="text-xs text-muted-foreground mt-0.5">
                {opt.disabled ? opt.disabledReason : opt.description}
              </p>
            </div>
          </label>
        ))}
      </div>

      {current === "metal" && metalAvailable === false && (
        <p className="text-xs text-amber-600 dark:text-amber-400">
          Metal is unavailable on this device. Transcription will use the CPU until you select Auto
          or CPU Only.
        </p>
      )}
    </div>
  );
}
