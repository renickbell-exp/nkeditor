/**
 * File Manager Service
 *
 * Handles preset storage, loading, and management.
 * Stores presets as JSON files in the user's data directory.
 */

import { app } from 'electron';
import { promises as fs, mkdirSync, existsSync } from 'fs';
import { join, basename } from 'path';
import type {
  Preset,
  PresetMetadata,
  SavePresetResponse,
  LoadPresetResponse,
  DeletePresetResponse,
  ListPresetsResponse,
} from '@shared/ipc-protocol.js';
import { validatePreset, safeValidatePreset, createSamplePresets } from '@shared/schemas.js';

// =============================================================================
// FileManager Class
// =============================================================================

/**
 * FileManager handles preset file operations.
 */
export class FileManager {
  private readonly presetsDir: string;
  private presetCache: Map<string, PresetMetadata> = new Map();
  private cacheInitialized = false;
  private samplePresetsInitialized = false;

  constructor() {
    // Use Electron's user data path for cross-platform compatibility
    const userDataPath = app.getPath('userData');
    this.presetsDir = join(userDataPath, 'presets');

    // Ensure presets directory exists
    this.ensurePresetsDir();
  }

  // ===========================================================================
  // Public Methods
  // ===========================================================================

  /**
   * Saves a preset to disk.
   */
  async savePreset(preset: Preset, overwrite = false): Promise<SavePresetResponse> {
    try {
      // Validate preset
      validatePreset(preset);

      const filePath = this.getPresetPath(preset.metadata.id);

      // Check if file exists and overwrite is not allowed
      if (!overwrite) {
        try {
          await fs.access(filePath);
          return { success: false, error: 'Preset already exists. Use overwrite option to replace.' };
        } catch {
          // File doesn't exist, proceed with save
        }
      }

      // Update modified timestamp
      preset.metadata.modifiedAt = new Date().toISOString();

      // Write to disk
      await fs.writeFile(filePath, JSON.stringify(preset, null, 2), 'utf-8');

      // Update cache
      this.presetCache.set(preset.metadata.id, preset.metadata);

      return { success: true, id: preset.metadata.id };
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      return { success: false, error: message };
    }
  }

  /**
   * Loads a preset from disk.
   */
  async loadPreset(id: string): Promise<LoadPresetResponse> {
    try {
      const filePath = this.getPresetPath(id);

      const content = await fs.readFile(filePath, 'utf-8');
      const data = JSON.parse(content);

      // Validate preset
      const result = safeValidatePreset(data);
      if (!result.success) {
        return { success: false, error: `Invalid preset format: ${result.error}` };
      }

      return { success: true, preset: result.data };
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code === 'ENOENT') {
        return { success: false, error: 'Preset not found' };
      }
      const message = error instanceof Error ? error.message : String(error);
      return { success: false, error: message };
    }
  }

  /**
   * Deletes a preset from disk.
   */
  async deletePreset(id: string): Promise<DeletePresetResponse> {
    try {
      const filePath = this.getPresetPath(id);

      await fs.unlink(filePath);

      // Remove from cache
      this.presetCache.delete(id);

      return { success: true };
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code === 'ENOENT') {
        return { success: false, error: 'Preset not found' };
      }
      const message = error instanceof Error ? error.message : String(error);
      return { success: false, error: message };
    }
  }

  /**
   * Lists all presets.
   */
  async listPresets(): Promise<ListPresetsResponse> {
    await this.refreshCache();
    const presets = Array.from(this.presetCache.values());
    return { presets };
  }

  /**
   * Gets preset metadata by ID.
   */
  async getPresetMetadata(id: string): Promise<PresetMetadata | null> {
    await this.refreshCache();
    return this.presetCache.get(id) ?? null;
  }

  // ===========================================================================
  // Private Methods
  // ===========================================================================

  /**
   * Gets the file path for a preset ID.
   */
  private getPresetPath(id: string): string {
    // Sanitize ID to prevent path traversal
    const safeId = id.replace(/[^a-zA-Z0-9_-]/g, '_');
    return join(this.presetsDir, `${safeId}.json`);
  }

  /**
   * Ensures the presets directory exists.
   * Uses synchronous operations to avoid race conditions during construction.
   */
  private ensurePresetsDir(): void {
    try {
      if (!existsSync(this.presetsDir)) {
        mkdirSync(this.presetsDir, { recursive: true });
      }
    } catch (error) {
      console.error('[file-manager] Failed to create presets directory:', error);
    }
  }

  /**
   * Refreshes the preset metadata cache.
   * On first initialization, creates sample presets if the directory is empty.
   */
  private async refreshCache(): Promise<void> {
    if (this.cacheInitialized) {
      // Only do a quick refresh if already initialized
      await this.quickRefreshCache();
      return;
    }

    try {
      this.ensurePresetsDir();

      const files = await fs.readdir(this.presetsDir);
      const jsonFiles = files.filter(f => f.endsWith('.json'));

      // Initialize sample presets on first run when directory is empty
      if (jsonFiles.length === 0 && !this.samplePresetsInitialized) {
        await this.initializeSamplePresets();
        this.samplePresetsInitialized = true;
        // Re-read directory after creating sample presets
        const updatedFiles = await fs.readdir(this.presetsDir);
        jsonFiles.push(...updatedFiles.filter(f => f.endsWith('.json')));
      }

      this.presetCache.clear();

      for (const file of jsonFiles) {
        const filePath = join(this.presetsDir, file);
        try {
          const content = await fs.readFile(filePath, 'utf-8');
          const data = JSON.parse(content);

          const result = safeValidatePreset(data);
          if (result.success) {
            this.presetCache.set(result.data.metadata.id, result.data.metadata);
          } else {
            console.warn(`[file-manager] Invalid preset file: ${file}`);
          }
        } catch (error) {
          console.warn(`[file-manager] Failed to read preset file: ${file}`, error);
        }
      }

      this.cacheInitialized = true;
    } catch (error) {
      console.error('[file-manager] Failed to refresh preset cache:', error);
    }
  }

  /**
   * Creates sample presets for first-run experience.
   * These provide useful starting points for common use cases.
   */
  private async initializeSamplePresets(): Promise<void> {
    console.log('[file-manager] Initializing sample presets for first run...');

    const samplePresets = createSamplePresets();

    for (const preset of samplePresets) {
      try {
        const filePath = this.getPresetPath(preset.metadata.id);
        await fs.writeFile(filePath, JSON.stringify(preset, null, 2), 'utf-8');
        console.log(`[file-manager] Created sample preset: ${preset.metadata.name}`);
      } catch (error) {
        console.error(`[file-manager] Failed to create sample preset ${preset.metadata.name}:`, error);
      }
    }

    console.log(`[file-manager] Sample presets initialization complete (${samplePresets.length} presets created)`);
  }

  /**
   * Quick refresh - just check for new/deleted files.
   */
  private async quickRefreshCache(): Promise<void> {
    try {
      const files = await fs.readdir(this.presetsDir);
      const jsonFiles = new Set(files.filter(f => f.endsWith('.json')).map(f => basename(f, '.json')));

      // Remove deleted presets from cache
      for (const id of this.presetCache.keys()) {
        const safeId = id.replace(/[^a-zA-Z0-9_-]/g, '_');
        if (!jsonFiles.has(safeId)) {
          this.presetCache.delete(id);
        }
      }

      // Add new presets (not in cache)
      for (const safeId of jsonFiles) {
        const cached = Array.from(this.presetCache.keys()).find(
          id => id.replace(/[^a-zA-Z0-9_-]/g, '_') === safeId
        );

        if (!cached) {
          const filePath = join(this.presetsDir, `${safeId}.json`);
          try {
            const content = await fs.readFile(filePath, 'utf-8');
            const data = JSON.parse(content);

            const result = safeValidatePreset(data);
            if (result.success) {
              this.presetCache.set(result.data.metadata.id, result.data.metadata);
            }
          } catch (error) {
            // Ignore errors on quick refresh
          }
        }
      }
    } catch (error) {
      // Ignore errors on quick refresh
    }
  }
}
