import { AccelerationSettings } from "@/components/settings/AccelerationSettings";

export function SettingsPage() {
  return (
    <div className="flex flex-col h-full overflow-auto">
      <div className="flex-none bg-background border-b border-border px-6 py-4 pt-10">
        <h1 className="text-lg font-semibold">Settings</h1>
      </div>

      <div className="flex-1 px-6 py-6 space-y-8 max-w-lg">
        <section>
          <AccelerationSettings />
        </section>
      </div>
    </div>
  );
}
