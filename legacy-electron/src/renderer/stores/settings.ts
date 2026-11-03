/**
 * Settings Store
 *
 * Manages application settings and configuration.
 */

import { create } from 'zustand';
import type { ElectronAPI } from '../../main/preload';
import type { AppConfig } from '@shared/ipc-protocol';

// Get the electron API from window
const getElectronAPI = (): ElectronAPI | null => {
  return (window as { electronAPI?: ElectronAPI }).electronAPI ?? null;
};

// =============================================================================
// Store Types
// =============================================================================

interface SettingsStoreState {
  // State
  config: AppConfig | null;
  loading: boolean;
  error: string | null;

  // Actions
  initialize: () => Promise<void>;
  updateConfig: (updates: Partial<AppConfig>) => Promise<void>;
  setQuickAccessSlot: (index: number, presetId: string | null) => Promise<void>;
}

// =============================================================================
// Store Implementation
// =============================================================================

export const useSettingsStore = create<SettingsStoreState>((set, get) => ({
  // Initial state
  config: null,
  loading: false,
  error: null,

  // Initialize settings
  initialize: async () => {
    const api = getElectronAPI();
    if (!api) return;

    set({ loading: true, error: null });

    try {
      const response = await api.getConfig();
      set({ config: response.config, loading: false });
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      set({ error: message, loading: false });
    }
  },

  // Update configuration
  updateConfig: async (updates) => {
    const api = getElectronAPI();
    if (!api) return;

    const currentConfig = get().config;
    if (!currentConfig) return;

    // Optimistic update
    set({ config: { ...currentConfig, ...updates } });

    try {
      const response = await api.updateConfig({ updates });

      if (response.success && response.config) {
        set({ config: response.config });
      } else {
        // Revert on failure
        set({ config: currentConfig, error: response.error ?? 'Failed to update config' });
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      set({ config: currentConfig, error: message });
    }
  },

  // Set a quick access slot
  setQuickAccessSlot: async (index, presetId) => {
    const currentConfig = get().config;
    if (!currentConfig) return;

    const newSlots = [...currentConfig.quickAccessSlots];
    newSlots[index] = presetId;

    await get().updateConfig({ quickAccessSlots: newSlots });
  },
}));
