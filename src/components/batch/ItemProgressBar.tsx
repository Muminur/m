export function ItemProgressBar({ progress }: { progress: number }) {
  const clamped = Math.max(0, Math.min(100, progress));
  return (
    <div className="w-full bg-gray-200 dark:bg-gray-700 rounded-full h-1.5 mt-1">
      <div
        className="bg-blue-500 h-1.5 rounded-full transition-all duration-300"
        style={{ width: `${clamped}%` }}
        role="progressbar"
        aria-valuenow={clamped}
        aria-valuemin={0}
        aria-valuemax={100}
      />
    </div>
  );
}
