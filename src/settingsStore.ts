import { create } from "zustand";
import { createStore } from "zustand/vanilla";
import * as api from "./api";
import { normalizeRemoteError } from "./errors";
import type { RemoteOpsError } from "./errors";
import { DEFAULT_SETTINGS, patchSettings } from "./settings";
import type { AppSettings, SettingsPatch } from "./settings";

interface SettingsDependencies {
  load: () => Promise<AppSettings>;
  save: (settings: AppSettings) => Promise<AppSettings>;
}

export interface SettingsState {
  settings: AppSettings;
  persisted: AppSettings;
  loading: boolean;
  initialized: boolean;
  saving: boolean;
  dirty: boolean;
  error: RemoteOpsError | null;
  load: () => Promise<void>;
  patch: (patch: SettingsPatch) => boolean;
  reset: () => boolean;
  save: () => Promise<void>;
}

const cloneSettings = (settings: Readonly<AppSettings>): AppSettings => ({
  ...settings,
  default_ports: { ...settings.default_ports },
});

const sameSettings = (a: AppSettings, b: AppSettings): boolean => JSON.stringify(a) === JSON.stringify(b);

function settingsCreator(dependencies: SettingsDependencies) {
  let loaded = false;
  let loadPromise: Promise<void> | null = null;
  let savePromise: Promise<void> | null = null;

  return (set: (partial: Partial<SettingsState> | ((state: SettingsState) => Partial<SettingsState>)) => void, get: () => SettingsState): SettingsState => {
    const initial = cloneSettings(DEFAULT_SETTINGS);
    return {
      settings: initial,
      persisted: cloneSettings(initial),
      loading: false,
      initialized: false,
      saving: false,
      dirty: false,
      error: null,
      load: () => {
        if (loaded) return Promise.resolve();
        if (loadPromise) return loadPromise;
        set({ loading: true, error: null });
        loadPromise = Promise.resolve().then(dependencies.load)
          .then((settings) => {
            const persisted = cloneSettings(settings);
            loaded = true;
            set({ settings: cloneSettings(persisted), persisted, loading: false, initialized: true, dirty: false });
          })
          .catch((rejection: unknown) => {
            const error = normalizeRemoteError(rejection);
            set({ loading: false, initialized: true, error });
            throw error;
          })
          .finally(() => { loadPromise = null; });
        return loadPromise;
      },
      patch: (patch) => {
        if (get().loading || get().saving) return false;
        set((state) => {
          const settings = patchSettings(state.settings, patch);
          return { settings, dirty: !sameSettings(settings, state.persisted), error: null };
        });
        return true;
      },
      reset: () => {
        if (get().loading || get().saving) return false;
        set((state) => ({
          settings: cloneSettings(state.persisted),
          dirty: false,
          error: null,
        }));
        return true;
      },
      save: () => {
        if (savePromise) return savePromise;
        const state = get();
        if (!state.dirty || state.loading || state.saving) return Promise.resolve();
        const snapshot = cloneSettings(state.persisted);
        const pending = cloneSettings(state.settings);
        set({ saving: true, error: null });
        savePromise = Promise.resolve().then(() => dependencies.save(pending))
          .then((settings) => {
            const persisted = cloneSettings(settings);
            set({ settings: cloneSettings(persisted), persisted, dirty: false, saving: false });
          })
          .catch((rejection: unknown) => {
            const error = normalizeRemoteError(rejection);
            set({
              settings: cloneSettings(snapshot),
              persisted: snapshot,
              dirty: false,
              saving: false,
              error,
            });
            throw error;
          })
          .finally(() => { savePromise = null; });
        return savePromise;
      },
    };
  };
}

export function createSettingsState(dependencies: SettingsDependencies) {
  return createStore<SettingsState>(settingsCreator(dependencies));
}

export const useSettingsStore = create<SettingsState>(settingsCreator({
  load: api.settingsGet,
  save: api.settingsSave,
}));
