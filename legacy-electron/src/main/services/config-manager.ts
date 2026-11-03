/**
 * Config Manager Service
 *
 * Manages application configuration persistence.
 * Stores settings in the user's data directory.
 */

import { app } from 'electron';
import { readFileSync, writeFileSync, existsSync, mkdirSync } from 'fs';
import { join, dirname } from 'path';
import type { AppConfig } from '@shared/ipc-protocol.js';
import { appConfigSchema, createDefaultAppConfig } from '@shared/schemas.js';

// =============================================================================
// ConfigManager Class
// =============================================================================

/**
 * ConfigManager handles application configuration persistence.
 */
export class ConfigManager {
  private readonly configPath: string;
  private config: AppConfig;
  private dirty = false;
  private saveIntervalId: NodeJS.Timeout | null = null;

  constructor() {
    // Use Electron's user data path
    const userDataPath = app.getPath('userData');
    this.configPath = join(userDataPath, 'config.json');

    // Load or create default config
    this.config = this.load();

    // Auto-save periodically
    this.saveIntervalId = setInterval(() => {
      if (this.dirty) {
        this.save();
      }
    }, 30000); // Save every 30 seconds if dirty
  }

  // ===========================================================================
  // Lifecycle Methods
  // ===========================================================================

  /**
   * Cleans up resources held by the ConfigManager.
   * Clears the auto-save interval and persists any pending changes.
   */
  cleanup(): void {
    if (this.saveIntervalId !== null) {
      clearInterval(this.saveIntervalId);
      this.saveIntervalId = null;
    }

    // Persist any unsaved changes before shutdown
    if (this.dirty) {
      this.save();
    }
  }

  // ===========================================================================
  // Public Methods
  // ===========================================================================

  /**
   * Gets the current configuration.
   */
  getConfig(): AppConfig {
    return { ...this.config };
  }

  /**
   * Updates the configuration with partial values.
   */
  update(updates: Partial<AppConfig>): AppConfig {
    // Merge updates
    const merged = { ...this.config, ...updates };

    // Validate
    const validated = appConfigSchema.parse(merged);

    // Update internal state
    this.config = validated;
    this.dirty = true;

    // Save immediately for important changes
    this.save();

    return this.getConfig();
  }

  /**
   * Resets configuration to defaults.
   */
  reset(): AppConfig {
    this.config = createDefaultAppConfig();
    this.dirty = true;
    this.save();
    return this.getConfig();
  }

  /**
   * Saves configuration to disk.
   */
  save(): void {
    try {
      // Ensure directory exists
      const dir = dirname(this.configPath);
      if (!existsSync(dir)) {
        mkdirSync(dir, { recursive: true });
      }

      writeFileSync(this.configPath, JSON.stringify(this.config, null, 2), 'utf-8');
      this.dirty = false;
      console.log('[config-manager] Configuration saved');
    } catch (error) {
      console.error('[config-manager] Failed to save configuration:', error);
    }
  }

  // ===========================================================================
  // Specific Getters/Setters
  // ===========================================================================

  /**
   * Gets soft takeover mode.
   */
  getSoftTakeoverMode(): AppConfig['softTakeoverMode'] {
    return this.config.softTakeoverMode;
  }

  /**
   * Sets soft takeover mode.
   */
  setSoftTakeoverMode(mode: AppConfig['softTakeoverMode']): void {
    this.update({ softTakeoverMode: mode });
  }

  /**
   * Gets soft takeover threshold.
   */
  getSoftTakeoverThreshold(): number {
    return this.config.softTakeoverThreshold;
  }

  /**
   * Sets soft takeover threshold.
   */
  setSoftTakeoverThreshold(threshold: number): void {
    this.update({ softTakeoverThreshold: threshold });
  }

  /**
   * Gets theme setting.
   */
  getTheme(): AppConfig['theme'] {
    return this.config.theme;
  }

  /**
   * Sets theme.
   */
  setTheme(theme: AppConfig['theme']): void {
    this.update({ theme: theme });
  }

  /**
   * Adds a preset to recent presets.
   */
  addRecentPreset(presetId: string): void {
    const recent = this.config.recentPresets.filter(id => id !== presetId);
    recent.unshift(presetId);
    if (recent.length > 10) {
      recent.pop();
    }
    this.update({ recentPresets: recent });
  }

  /**
   * Gets quick access slots.
   */
  getQuickAccessSlots(): (string | null)[] {
    return [...this.config.quickAccessSlots];
  }

  /**
   * Sets a quick access slot.
   */
  setQuickAccessSlot(index: number, presetId: string | null): void {
    if (index < 0 || index >= 5) {
      throw new Error('Quick access slot index must be 0-4');
    }
    const slots = [...this.config.quickAccessSlots];
    slots[index] = presetId;
    this.update({ quickAccessSlots: slots });
  }

  /**
   * Saves window bounds for restoration.
   */
  setWindowBounds(bounds: AppConfig['windowBounds']): void {
    this.update({ windowBounds: bounds });
  }

  /**
   * Gets window bounds.
   */
  getWindowBounds(): AppConfig['windowBounds'] {
    return this.config.windowBounds;
  }

  // ===========================================================================
  // Private Methods
  // ===========================================================================

  /**
   * Loads configuration from disk or creates default.
   */
  private load(): AppConfig {
    try {
      if (existsSync(this.configPath)) {
        const content = readFileSync(this.configPath, 'utf-8');
        const data = JSON.parse(content);

        // Validate and merge with defaults (for new fields)
        const defaults = createDefaultAppConfig();
        const merged = { ...defaults, ...data };

        return appConfigSchema.parse(merged);
      }
    } catch (error) {
      console.warn('[config-manager] Failed to load config, using defaults:', error);
    }

    // Return default config
    return createDefaultAppConfig();
  }
}
