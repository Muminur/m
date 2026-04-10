import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";

export interface UpdateInfo {
  version: string;
  body?: string;
  date?: string;
}

interface UpdateState {
  appVersion: string | null;
  update: UpdateInfo | null;
  checking: boolean;
  installing: boolean;
  error: string | null;

  loadVersion: () => Promise<void>;
  checkForUpdate: () => Promise<void>;
  installUpdate: () => Promise<void>;
}

export const useUpdateStore = create<UpdateState>((set) => ({
  appVersion: null,
  update: null,
  checking: false,
  installing: false,
  error: null,

  loadVersion: async () => {
    try {
      const version = await invoke<string>("get_app_version");
      set({ appVersion: version });
    } catch (err) {
      set({ error: String(err) });
    }
  },

  checkForUpdate: async () => {
    set({ checking: true, error: null });
    try {
      const update = await invoke<UpdateInfo | null>("check_for_update");
      set({ update, checking: false });
    } catch (err) {
      set({ error: String(err), checking: false });
    }
  },

  installUpdate: async () => {
    set({ installing: true, error: null });
    try {
      await invoke("download_and_install_update");
      set({ installing: false });
    } catch (err) {
      set({ error: String(err), installing: false });
    }
  },
}));
