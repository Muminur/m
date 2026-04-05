import { Minus, Plus, Users } from "lucide-react";

interface SpeakerCountHintProps {
  value: number;
  onChange: (count: number) => void;
  min?: number;
  max?: number;
}

/**
 * Number input with +/- buttons for specifying expected speaker count
 * before transcription begins.
 */
export function SpeakerCountHint({
  value,
  onChange,
  min = 1,
  max = 10,
}: SpeakerCountHintProps) {
  const handleDecrement = () => {
    if (value > min) {
      onChange(value - 1);
    }
  };

  const handleIncrement = () => {
    if (value < max) {
      onChange(value + 1);
    }
  };

  const handleInputChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const parsed = parseInt(e.target.value, 10);
    if (!isNaN(parsed)) {
      const clamped = Math.max(min, Math.min(max, parsed));
      onChange(clamped);
    }
  };

  return (
    <div
      data-testid="speaker-count-hint"
      className="flex items-center gap-3"
    >
      <div className="flex items-center gap-1.5 text-sm text-muted-foreground">
        <Users size={14} />
        <span>Expected Speakers</span>
      </div>

      <div className="flex items-center gap-1">
        <button
          data-testid="speaker-count-dec"
          onClick={handleDecrement}
          disabled={value <= min}
          aria-label="Decrease speaker count"
          className="w-7 h-7 flex items-center justify-center rounded-md border border-border bg-background hover:bg-accent transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
        >
          <Minus size={12} />
        </button>

        <input
          data-testid="speaker-count-input"
          type="number"
          value={value}
          onChange={handleInputChange}
          min={min}
          max={max}
          aria-label="Speaker count"
          className="w-12 h-7 text-center text-sm border border-border rounded-md bg-background focus:outline-none focus:ring-1 focus:ring-ring [appearance:textfield] [&::-webkit-outer-spin-button]:appearance-none [&::-webkit-inner-spin-button]:appearance-none"
        />

        <button
          data-testid="speaker-count-inc"
          onClick={handleIncrement}
          disabled={value >= max}
          aria-label="Increase speaker count"
          className="w-7 h-7 flex items-center justify-center rounded-md border border-border bg-background hover:bg-accent transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
        >
          <Plus size={12} />
        </button>
      </div>
    </div>
  );
}
