/**
 * Settings View Component
 *
 * Application settings and configuration UI.
 */

import React, { useState, useCallback } from 'react';
import { useSettingsStore } from '../../stores/settings';
import { useConnectionStore } from '../../stores/connection';
import { DEFAULT_THEME_COLORS } from '@shared/ipc-protocol';
import type { ThemeColors, UserTheme } from '@shared/ipc-protocol';

export function SettingsView(): React.ReactElement {
  const config = useSettingsStore((state) => state.config);
  const updateConfig = useSettingsStore((state) => state.updateConfig);
  const { connected, inputPort, midiAvailable } = useConnectionStore();
  const connect = useConnectionStore((state) => state.connect);
  const disconnect = useConnectionStore((state) => state.disconnect);

  if (!config) {
    return (
      <div className="p-4">
        <div className="card p-4">Loading settings...</div>
      </div>
    );
  }

  return (
    <div className="h-full p-4 overflow-auto">
      <div className="max-w-2xl mx-auto space-y-6">
        {/* MIDI Settings */}
        <section className="card p-4">
          <h2 className="text-lg font-semibold text-gray-200 mb-4">MIDI Connection</h2>

          <div className="space-y-4">
            {/* Connection status */}
            <div className="flex items-center justify-between p-3 bg-nk-darker rounded">
              <div>
                <div className="font-medium text-gray-200">Status</div>
                <div className="text-sm text-gray-500">
                  {connected ? `Connected to ${inputPort}` : 'Not connected'}
                </div>
              </div>
              <div className="flex gap-2">
                {connected ? (
                  <button className="btn btn-secondary" onClick={disconnect}>
                    Disconnect
                  </button>
                ) : (
                  <button
                    className="btn btn-primary"
                    onClick={() => connect()}
                    disabled={!midiAvailable}
                  >
                    Connect
                  </button>
                )}
              </div>
            </div>

            {/* Auto-connect */}
            <SettingToggle
              label="Auto-connect on startup"
              description="Automatically connect to nanoKONTROL2 when the app starts"
              checked={config.autoConnect}
              onChange={(checked) => updateConfig({ autoConnect: checked })}
            />

            {/* Global MIDI Channel */}
            <div className="p-3 bg-nk-darker rounded space-y-2">
              <label className="block text-sm font-medium text-gray-300">
                Default MIDI Channel
              </label>
              <select
                value={config.globalMidiChannel}
                onChange={(e) => updateConfig({ globalMidiChannel: parseInt(e.target.value, 10) })}
                className="input w-24"
              >
                {Array.from({ length: 16 }, (_, i) => (
                  <option key={i + 1} value={i + 1}>{i + 1}</option>
                ))}
              </select>
              <p className="text-xs text-gray-500">
                Used as default channel when creating new mappings
              </p>
            </div>

            {/* LED Mode */}
            <div className="p-3 bg-nk-darker rounded space-y-2">
              <label className="block text-sm font-medium text-gray-300">
                LED Mode
              </label>
              <select
                value={config.ledMode}
                onChange={(e) => updateConfig({ ledMode: e.target.value as 'internal' | 'external' })}
                className="input w-32"
              >
                <option value="internal">Internal</option>
                <option value="external">External</option>
              </select>
              <p className="text-xs text-gray-500">
                Internal: LEDs reflect button state. External: LEDs controlled by DAW.
              </p>
            </div>
          </div>
        </section>

        {/* Preset Loading Settings */}
        <section className="card p-4">
          <h2 className="text-lg font-semibold text-gray-200 mb-4">Preset Loading</h2>

          <div className="space-y-4">
            {/* Button Transmission */}
            <div className="space-y-2">
              <label className="flex items-start gap-3 cursor-pointer">
                <input
                  type="checkbox"
                  className="mt-1"
                  checked={config.transmitButtonsOnLoad}
                  onChange={(e) => updateConfig({ transmitButtonsOnLoad: e.target.checked })}
                />
                <div>
                  <span className="text-sm text-gray-200">Transmit button states on preset load</span>
                  <p className="text-xs text-gray-500 mt-0.5">
                    When disabled, only knob and slider values are sent to synth.
                    Button states still update in the GUI. This can help if button
                    MIDI messages are causing issues with your synth or DAW.
                  </p>
                </div>
              </label>
            </div>
          </div>
        </section>

        {/* Soft Takeover Settings */}
        <section className="card p-4">
          <h2 className="text-lg font-semibold text-gray-200 mb-4">Soft Takeover</h2>

          <div className="space-y-4">
            {/* Mode selection */}
            <div>
              <label className="block text-sm font-medium text-gray-300 mb-2">
                Mode
              </label>
              <select
                className="input w-full"
                value={config.softTakeoverMode}
                onChange={(e) =>
                  updateConfig({
                    softTakeoverMode: e.target.value as 'catch' | 'jump' | 'pickup',
                  })
                }
              >
                <option value="catch">
                  Catch - Output only when physical value matches target
                </option>
                <option value="jump">
                  Jump - Immediately output physical value (no soft takeover)
                </option>
                <option value="pickup">
                  Pickup - Output only when physical crosses target
                </option>
              </select>
            </div>

            {/* Threshold */}
            <div>
              <label className="block text-sm font-medium text-gray-300 mb-2">
                Threshold: {config.softTakeoverThreshold}
              </label>
              <input
                type="range"
                min="1"
                max="20"
                value={config.softTakeoverThreshold}
                onChange={(e) =>
                  updateConfig({ softTakeoverThreshold: parseInt(e.target.value, 10) })
                }
                className="w-full"
              />
              <p className="text-xs text-gray-500 mt-1">
                How close the physical value must be to the target to "catch" it
              </p>
            </div>
          </div>
        </section>

        {/* Appearance Settings */}
        <section className="card p-4">
          <h2 className="text-lg font-semibold text-gray-200 mb-4">Appearance</h2>

          <div className="space-y-4">
            {/* Value display */}
            <div>
              <label className="block text-sm font-medium text-gray-300 mb-2">
                Value Display
              </label>
              <select
                className="input w-full"
                value={config.valueDisplay}
                onChange={(e) =>
                  updateConfig({ valueDisplay: e.target.value as 'decimal' | 'hex' })
                }
              >
                <option value="decimal">Decimal (0-127)</option>
                <option value="hex">Hexadecimal (00-7F)</option>
              </select>
            </div>

            {/* UI Scale */}
            <div>
              <label className="block text-sm font-medium text-gray-300 mb-2">
                UI Scale: {config.uiScale ?? 100}%
              </label>
              <input
                type="range"
                min="50"
                max="200"
                step="10"
                value={config.uiScale ?? 100}
                onChange={(e) => updateConfig({ uiScale: parseInt(e.target.value, 10) })}
                className="w-full"
              />
              <div className="flex justify-between text-xs text-gray-500 mt-1">
                <span>50%</span>
                <span>100%</span>
                <span>200%</span>
              </div>
            </div>

            {/* Font Family */}
            <div>
              <label className="block text-sm font-medium text-gray-300 mb-2">
                Font Family
              </label>
              <select
                className="input w-full"
                value={config.fontFamily ?? 'Inter'}
                onChange={(e) => updateConfig({ fontFamily: e.target.value })}
              >
                <option value="Inter">Inter</option>
                <option value="Roboto">Roboto</option>
                <option value="Arial">Arial</option>
                <option value="Helvetica">Helvetica</option>
                <option value="system-ui">System UI</option>
                <option value="Segoe UI">Segoe UI</option>
                <option value="SF Pro Display">SF Pro Display</option>
                <option value="Verdana">Verdana</option>
                <option value="Tahoma">Tahoma</option>
              </select>
            </div>

            {/* Font Size */}
            <div>
              <label className="block text-sm font-medium text-gray-300 mb-2">
                Font Size: {config.fontSize ?? 14}px
              </label>
              <input
                type="range"
                min="10"
                max="24"
                step="1"
                value={config.fontSize ?? 14}
                onChange={(e) => updateConfig({ fontSize: parseInt(e.target.value, 10) })}
                className="w-full"
              />
              <div className="flex justify-between text-xs text-gray-500 mt-1">
                <span>10px</span>
                <span>14px</span>
                <span>24px</span>
              </div>
            </div>
          </div>
        </section>

        {/* Theme Colors */}
        <ThemeColorEditor config={config} updateConfig={updateConfig} />

        {/* About */}
        <section className="card p-4">
          <h2 className="text-lg font-semibold text-gray-200 mb-4">About</h2>
          <div className="text-sm text-gray-400">
            <p>nkEditor3 v2.0.0</p>
            <p className="mt-2">
              A MIDI CC remapper for the Korg nanoKONTROL2 hardware controller.
            </p>
            <p className="mt-2">
              Built with Electron, React, and TypeScript.
            </p>
          </div>
        </section>
      </div>
    </div>
  );
}

// =============================================================================
// Sub-components
// =============================================================================

interface SettingToggleProps {
  label: string;
  description: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
}

function SettingToggle({ label, description, checked, onChange }: SettingToggleProps): React.ReactElement {
  return (
    <div className="flex items-center justify-between p-3 bg-nk-darker rounded">
      <div>
        <div className="font-medium text-gray-200">{label}</div>
        <div className="text-sm text-gray-500">{description}</div>
      </div>
      <button
        className={`
          w-12 h-6 rounded-full transition-colors relative
          ${checked ? 'bg-nk-accent' : 'bg-gray-600'}
        `}
        onClick={() => onChange(!checked)}
      >
        <span
          className={`
            absolute top-1 w-4 h-4 rounded-full bg-white transition-transform
            ${checked ? 'translate-x-7' : 'translate-x-1'}
          `}
        />
      </button>
    </div>
  );
}

// =============================================================================
// Color Input Component
// =============================================================================

interface ColorInputProps {
  label: string;
  value: string;
  onChange: (value: string) => void;
}

function ColorInput({ label, value, onChange }: ColorInputProps): React.ReactElement {
  return (
    <div className="flex items-center gap-3 p-2 bg-nk-darker rounded">
      <input
        type="color"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className="w-8 h-8 rounded cursor-pointer border border-nk-border bg-transparent"
      />
      <div className="flex-1">
        <div className="text-sm text-gray-300">{label}</div>
        <input
          type="text"
          value={value}
          onChange={(e) => {
            const v = e.target.value;
            if (/^#[0-9a-fA-F]{6}$/.test(v)) {
              onChange(v);
            }
          }}
          className="input text-xs py-0.5 px-1 w-24 mt-0.5 font-mono"
          maxLength={7}
          placeholder="#000000"
        />
      </div>
    </div>
  );
}

// =============================================================================
// Theme Color Editor Component
// =============================================================================

interface ThemeColorEditorProps {
  config: import('@shared/ipc-protocol').AppConfig;
  updateConfig: (updates: Partial<import('@shared/ipc-protocol').AppConfig>) => Promise<void>;
}

function ThemeColorEditor({ config, updateConfig }: ThemeColorEditorProps): React.ReactElement {
  const [newThemeName, setNewThemeName] = useState('');

  const currentColors: ThemeColors = config.customThemeColors ?? { ...DEFAULT_THEME_COLORS };
  const userThemes: UserTheme[] = config.userThemes ?? [];

  const updateColor = useCallback((key: keyof ThemeColors, value: string) => {
    const updated = { ...currentColors, [key]: value };
    updateConfig({ customThemeColors: updated });
  }, [currentColors, updateConfig]);

  const resetToDefaults = useCallback(() => {
    updateConfig({ customThemeColors: undefined });
  }, [updateConfig]);

  const saveAsTheme = useCallback(() => {
    const name = newThemeName.trim();
    if (!name) return;
    const newTheme: UserTheme = { name, colors: { ...currentColors } };
    const existing = userThemes.filter((t) => t.name !== name);
    updateConfig({ userThemes: [...existing, newTheme] });
    setNewThemeName('');
  }, [newThemeName, currentColors, userThemes, updateConfig]);

  const loadTheme = useCallback((theme: UserTheme) => {
    updateConfig({ customThemeColors: { ...theme.colors } });
  }, [updateConfig]);

  const deleteTheme = useCallback((name: string) => {
    updateConfig({ userThemes: userThemes.filter((t) => t.name !== name) });
  }, [userThemes, updateConfig]);

  const colorFields: { key: keyof ThemeColors; label: string }[] = [
    { key: 'background', label: 'Background' },
    { key: 'backgroundDarker', label: 'Background (darker)' },
    { key: 'surface', label: 'Surface / Light' },
    { key: 'border', label: 'Border' },
    { key: 'accent', label: 'Accent' },
    { key: 'solo', label: 'Solo LED' },
    { key: 'mute', label: 'Mute LED' },
    { key: 'rec', label: 'Record LED' },
  ];

  return (
    <section className="card p-4">
      <h2 className="text-lg font-semibold text-gray-200 mb-4">Theme Colors</h2>

      <div className="space-y-4">
        {/* User themes selector */}
        {userThemes.length > 0 && (
          <div>
            <label className="block text-sm font-medium text-gray-300 mb-2">
              Saved Themes
            </label>
            <div className="flex flex-wrap gap-2">
              {userThemes.map((theme) => (
                <div key={theme.name} className="flex items-center gap-1">
                  <button
                    className="btn btn-ghost text-xs py-1 px-2 flex items-center gap-1.5"
                    onClick={() => loadTheme(theme)}
                    title={`Load theme: ${theme.name}`}
                  >
                    <span
                      className="w-3 h-3 rounded-full border border-gray-600"
                      style={{ backgroundColor: theme.colors.accent }}
                    />
                    {theme.name}
                  </button>
                  <button
                    className="text-xs text-red-400 hover:text-red-300 px-1"
                    onClick={() => deleteTheme(theme.name)}
                    title="Delete theme"
                  >
                    x
                  </button>
                </div>
              ))}
            </div>
          </div>
        )}

        {/* Color editors */}
        <div className="grid grid-cols-2 gap-2">
          {colorFields.map(({ key, label }) => (
            <ColorInput
              key={key}
              label={label}
              value={currentColors[key]}
              onChange={(v) => updateColor(key, v)}
            />
          ))}
        </div>

        {/* Actions */}
        <div className="flex gap-2 pt-2">
          <button
            className="btn btn-ghost text-sm"
            onClick={resetToDefaults}
          >
            Reset to Defaults
          </button>
          <div className="flex-1" />
          <input
            type="text"
            className="input text-sm w-36"
            placeholder="Theme name..."
            value={newThemeName}
            onChange={(e) => setNewThemeName(e.target.value)}
            maxLength={30}
          />
          <button
            className="btn btn-primary text-sm"
            onClick={saveAsTheme}
            disabled={!newThemeName.trim()}
          >
            Save Theme
          </button>
        </div>
      </div>
    </section>
  );
}
