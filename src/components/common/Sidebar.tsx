import { NavLink, useLocation } from "react-router-dom";
import { useState } from "react";
import {
  FileText,
  Mic,
  Sparkles,
  Radio,
  Globe,
  List,
  Settings,
  Star,
  Trash2,
  Download,
  Upload,
  Sun,
  Moon,
  Monitor,
  Info,
} from "lucide-react";
import { useSettingsStore } from "@/stores/settingsStore";
import { useTranslation } from "react-i18next";
import { AboutDialog } from "./AboutDialog";
import { formatError } from "@/lib/formatError";

export function Sidebar() {
  const { t } = useTranslation();
  const { settings, updateSettings } = useSettingsStore();
  const [aboutOpen, setAboutOpen] = useState(false);
  const location = useLocation();

  const cycleTheme = () => {
    const themes = ["light", "dark", "system"] as const;
    const current = settings?.theme ?? "system";
    const next = themes[(themes.indexOf(current) + 1) % themes.length];
    void updateSettings({ theme: next }).catch((error) => {
      console.error("Failed to update theme:", formatError(error));
    });
  };

  const ThemeIcon = settings?.theme === "dark" ? Moon : settings?.theme === "light" ? Sun : Monitor;

  // Determine which library sub-view is active by combining the pathname
  // with the ?filter query param. NavLink's built-in isActive matches by
  // pathname only, so all three /library?filter=* links highlighted at
  // once on first open. We compute the active flag explicitly below.
  const onLibraryPath =
    location.pathname === "/library" || location.pathname.startsWith("/library/");
  const filterParam = new URLSearchParams(location.search).get("filter");
  const libraryActive = onLibraryPath && !filterParam;
  const starredActive = onLibraryPath && filterParam === "starred";
  const trashActive = onLibraryPath && filterParam === "trash";

  return (
    <nav className="flex flex-col h-full min-h-0 overflow-y-auto pt-8 pb-3 px-2 gap-1 no-drag">
      {/* Navigation items */}
      <NavItem
        to="/library"
        icon={<FileText size={16} />}
        label={t("nav.library")}
        active={libraryActive}
      />
      <NavItem
        to="/library?filter=starred"
        icon={<Star size={16} />}
        label={t("nav.starred")}
        active={starredActive}
      />
      <NavItem to="/recording" icon={<Mic size={16} />} label={t("nav.recording")} />
      <NavItem to="/transcribe" icon={<Upload size={16} />} label={t("nav.transcribe")} />
      <NavItem to="/models" icon={<Download size={16} />} label={t("nav.models")} />
      <div className="text-[11px] uppercase tracking-wide px-3 pt-2 pb-1 text-muted-foreground">
        {t("nav.advanced")}
      </div>
      <NavItem to="/batch" icon={<List size={16} />} label={t("nav.batch")} />
      <NavItem to="/ai" icon={<Sparkles size={16} />} label={t("nav.ai")} />
      <NavItem to="/captions" icon={<Radio size={16} />} label={t("nav.captions")} />
      <NavItem to="/integrations" icon={<Globe size={16} />} label={t("nav.integrations")} />

      <div className="h-px bg-border my-2 mx-1" />

      <NavItem
        to="/library?filter=trash"
        icon={<Trash2 size={16} />}
        label={t("nav.trash")}
        active={trashActive}
      />

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

      <button
        onClick={() => setAboutOpen(true)}
        className="flex items-center gap-2 px-3 py-2 rounded-md text-sm text-muted-foreground hover:text-foreground hover:bg-accent transition-colors w-full text-left"
        title={t("about.title")}
      >
        <Info size={16} />
        <span>{t("about.title")}</span>
      </button>

      <AboutDialog open={aboutOpen} onClose={() => setAboutOpen(false)} />
    </nav>
  );
}

function NavItem({
  to,
  icon,
  label,
  active,
}: {
  to: string;
  icon: React.ReactNode;
  label: string;
  /** Override NavLink's built-in pathname-only active matching. When
   *  provided, this value wins; when undefined, NavLink computes from the
   *  current pathname (used for Recording/Models/Settings/etc.). */
  active?: boolean;
}) {
  return (
    <NavLink
      to={to}
      end
      className={({ isActive }) => {
        const isOn = active ?? isActive;
        return `flex items-center gap-2 px-3 py-2 rounded-md text-sm transition-colors ${
          isOn
            ? "bg-accent text-accent-foreground font-medium"
            : "text-muted-foreground hover:text-foreground hover:bg-accent"
        }`;
      }}
    >
      {icon}
      <span>{label}</span>
    </NavLink>
  );
}
