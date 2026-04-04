import { NavLink } from "react-router-dom";
import {
  FileText,
  Mic,
  Settings,
  Star,
  Trash2,
  Download,
  Sun,
  Moon,
  Monitor,
} from "lucide-react";
import { useSettingsStore } from "@/stores/settingsStore";
import { useTranslation } from "react-i18next";

export function Sidebar() {
  const { t } = useTranslation();
  const { settings, updateSettings } = useSettingsStore();

  const cycleTheme = () => {
    const themes = ["light", "dark", "system"] as const;
    const current = settings?.theme ?? "system";
    const next = themes[(themes.indexOf(current) + 1) % themes.length];
    updateSettings({ theme: next });
  };

  const ThemeIcon =
    settings?.theme === "dark" ? Moon : settings?.theme === "light" ? Sun : Monitor;

  return (
    <nav className="flex flex-col h-full pt-8 pb-3 px-2 gap-1 no-drag">
      {/* Navigation items */}
      <NavItem to="/library" icon={<FileText size={16} />} label={t("nav.library")} />
      <NavItem to="/library?filter=starred" icon={<Star size={16} />} label={t("nav.starred")} />
      <NavItem to="/recording" icon={<Mic size={16} />} label={t("nav.recording")} />
      <NavItem to="/models" icon={<Download size={16} />} label={t("nav.models")} />

      <div className="h-px bg-border my-2 mx-1" />

      <NavItem to="/library?filter=trash" icon={<Trash2 size={16} />} label={t("nav.trash")} />

      <div className="flex-1" />

      {/* Bottom actions */}
      <button
        onClick={cycleTheme}
        className="flex items-center gap-2 px-3 py-2 rounded-md text-sm text-muted-foreground hover:text-foreground hover:bg-accent transition-colors w-full text-left"
        title={t("settings.theme")}
      >
        <ThemeIcon size={16} />
        <span>{t(`settings.theme_${settings?.theme ?? "system"}`)}</span>
      </button>

      <NavItem to="/settings" icon={<Settings size={16} />} label={t("nav.settings")} />
    </nav>
  );
}

function NavItem({
  to,
  icon,
  label,
}: {
  to: string;
  icon: React.ReactNode;
  label: string;
}) {
  return (
    <NavLink
      to={to}
      className={({ isActive }) =>
        `flex items-center gap-2 px-3 py-2 rounded-md text-sm transition-colors ${
          isActive
            ? "bg-accent text-accent-foreground font-medium"
            : "text-muted-foreground hover:text-foreground hover:bg-accent"
        }`
      }
    >
      {icon}
      <span>{label}</span>
    </NavLink>
  );
}
