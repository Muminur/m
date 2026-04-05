import type { AudioDevice } from "@/lib/types";

interface DeviceSelectorProps {
  devices: AudioDevice[];
  selectedDeviceId: string | null;
  onSelect: (deviceId: string) => void;
  disabled?: boolean;
}

export function DeviceSelector({
  devices,
  selectedDeviceId,
  onSelect,
  disabled,
}: DeviceSelectorProps) {
  const inputDevices = devices.filter((d) => d.isInput);

  return (
    <div>
      <label className="text-sm font-medium mb-2 block">Input Device</label>
      <select
        value={selectedDeviceId ?? ""}
        onChange={(e) => onSelect(e.target.value)}
        disabled={disabled || inputDevices.length === 0}
        className="w-full px-3 py-2 rounded-md border border-border bg-background text-sm focus:outline-none focus:ring-2 focus:ring-ring disabled:opacity-50 disabled:cursor-not-allowed"
      >
        {inputDevices.length === 0 ? (
          <option value="">No input devices found</option>
        ) : (
          <>
            <option value="">Default device</option>
            {inputDevices.map((device) => (
              <option key={device.id} value={device.id}>
                {device.name}
                {device.isDefault ? " (Default)" : ""}
              </option>
            ))}
          </>
        )}
      </select>
    </div>
  );
}
