import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { CloudProviderInfo, CloudCostEstimate } from "@/lib/aiTypes";
import { Cloud, DollarSign, AlertTriangle } from "lucide-react";

interface CloudTranscriptionProps {
  filePath: string;
  enabled: boolean;
  onToggle: (enabled: boolean) => void;
  onProviderSelect: (provider: string) => void;
  selectedProvider: string;
}

export function CloudTranscription({
  filePath,
  enabled,
  onToggle,
  onProviderSelect,
  selectedProvider,
}: CloudTranscriptionProps) {
  const [providers, setProviders] = useState<CloudProviderInfo[]>([]);
  const [costEstimate, setCostEstimate] = useState<CloudCostEstimate | null>(null);

  useEffect(() => {
    invoke<CloudProviderInfo[]>("list_cloud_providers")
      .then(setProviders)
      .catch(() => setProviders([]));
  }, []);

  useEffect(() => {
    if (enabled && filePath && selectedProvider) {
      invoke<CloudCostEstimate>("estimate_cloud_cost", {
        filePath,
        provider: selectedProvider,
      })
        .then(setCostEstimate)
        .catch(() => setCostEstimate(null));
    }
  }, [enabled, filePath, selectedProvider]);

  const currentProvider = providers.find((p) => p.name === selectedProvider);

  return (
    <div className="space-y-3 rounded-md border border-border p-3">
      <div className="flex items-center gap-2">
        <input
          type="checkbox"
          id="cloud-transcription"
          checked={enabled}
          onChange={(e) => onToggle(e.target.checked)}
          className="rounded border-border"
        />
        <label
          htmlFor="cloud-transcription"
          className="flex items-center gap-1.5 text-sm font-medium cursor-pointer"
        >
          <Cloud className="h-4 w-4" />
          Use cloud transcription
        </label>
      </div>

      {enabled && (
        <div className="space-y-3 pl-6">
          <div>
            <label className="text-xs font-medium text-muted-foreground mb-1 block">
              Cloud Provider
            </label>
            <select
              value={selectedProvider}
              onChange={(e) => onProviderSelect(e.target.value)}
              className="w-full rounded-md border border-border bg-background px-3 py-2 text-sm"
            >
              {providers.map((p) => (
                <option key={p.name} value={p.name}>
                  {p.displayName} (${p.costPerMinuteUsd.toFixed(4)}/min)
                </option>
              ))}
            </select>
          </div>

          {/* Cost estimate */}
          {costEstimate && (
            <div className="flex items-center gap-1.5 text-xs text-muted-foreground bg-muted/50 rounded-md px-3 py-2">
              <DollarSign className="h-3.5 w-3.5" />
              <span>
                Est. {costEstimate.durationMinutes.toFixed(1)} min
              </span>
              <span className="ml-auto font-medium">
                ${costEstimate.estimatedUsd.toFixed(4)}
              </span>
            </div>
          )}

          {/* Disclaimer */}
          {currentProvider && (
            <div className="flex items-start gap-1.5 text-xs text-amber-600 dark:text-amber-400 bg-amber-50 dark:bg-amber-950/20 rounded-md px-3 py-2">
              <AlertTriangle className="h-3.5 w-3.5 mt-0.5 flex-shrink-0" />
              <span>
                Audio will be sent to{" "}
                <span className="font-medium">{currentProvider.displayName}</span> for
                processing. By proceeding, you consent to their data handling policies.
              </span>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
