/**
 * App Component
 *
 * Root component for the nkEditor3 application.
 * Manages layout and navigation between Editor, Librarian, Settings, and Help views.
 */

import React, { useState, useEffect } from 'react';
import { Header } from './components/Header';
import { EditorView } from './components/Editor/EditorView';
import { DeviceEditorContainer } from './components/Editor/DeviceEditorContainer';
import { DeviceTabBar } from './components/DeviceTabBar';
import { LibrarianView } from './components/Librarian/LibrarianView';
import { SettingsView } from './components/Settings/SettingsView';
import { HelpView } from './components/Help/HelpView';
import { useConnectionStore } from './stores/connection';
import { useSettingsStore } from './stores/settings';
import { useControlsStore } from './stores/controls';
import { usePresetStore } from './stores/preset';
import { useDevicesStore } from './stores/devices';
import { DEFAULT_THEME_COLORS } from '@shared/ipc-protocol';
import type { ThemeColors } from '@shared/ipc-protocol';

type TabId = 'editor' | 'librarian' | 'settings' | 'help';

function App(): React.ReactElement {
  const [activeTab, setActiveTab] = useState<TabId>('editor');
  const [windowWidth, setWindowWidth] = useState(window.innerWidth);

  // Initialize stores on mount
  const initConnection = useConnectionStore((state) => state.initialize);
  const initSettings = useSettingsStore((state) => state.initialize);
  const initControls = useControlsStore((state) => state.initialize);
  const loadPresets = usePresetStore((state) => state.loadPresets);
  const loadPreset = usePresetStore((state) => state.loadPreset);

  useEffect(() => {
    initConnection();
    initSettings();
    initControls();

    // Load presets on startup and auto-load the first one
    const initPresets = async () => {
      await loadPresets();
      const presets = usePresetStore.getState().presets;
      if (presets.length > 0 && presets[0]) {
        await loadPreset(presets[0].id);
      }
    };
    initPresets();
  }, [initConnection, initSettings, initControls, loadPresets, loadPreset]);

  // Apply theme colors, font, and UI scale from settings
  const config = useSettingsStore((state) => state.config);
  useEffect(() => {
    if (!config) return;
    const root = document.documentElement;
    const colors: ThemeColors = config.customThemeColors ?? DEFAULT_THEME_COLORS;
    root.style.setProperty('--nk-dark', colors.background);
    root.style.setProperty('--nk-darker', colors.backgroundDarker);
    root.style.setProperty('--nk-light', colors.surface);
    root.style.setProperty('--nk-border', colors.border);
    root.style.setProperty('--nk-accent', colors.accent);
    root.style.setProperty('--nk-solo', colors.solo);
    root.style.setProperty('--nk-mute', colors.mute);
    root.style.setProperty('--nk-rec', colors.rec);
    if (config.fontFamily) {
      root.style.setProperty('--nk-font-family', config.fontFamily);
    }
    if (config.fontSize) {
      root.style.setProperty('--nk-font-size', `${config.fontSize}px`);
    }
    if (config.uiScale) {
      root.style.setProperty('--nk-ui-scale', String(config.uiScale / 100));
    }
  }, [config]);

  // Track window width for layout decisions
  useEffect(() => {
    const handleResize = () => setWindowWidth(window.innerWidth);
    window.addEventListener('resize', handleResize);
    return () => window.removeEventListener('resize', handleResize);
  }, []);

  // Use split layout on wide screens (>= 1400px)
  const useSplitLayout = windowWidth >= 1400;

  const hasDevices = useDevicesStore((state) => state.devices.size > 0);

  // Determine which editor to show: multi-device container or legacy single editor
  const showDeviceTabBar = hasDevices && (activeTab === 'editor' || activeTab === 'librarian');

  /**
   * Renders the appropriate editor content.
   * When devices are configured, routes through DeviceEditorContainer.
   * Otherwise, falls back to the legacy single-device EditorView.
   */
  const renderEditorContent = () => {
    if (hasDevices) {
      return <DeviceEditorContainer />;
    }
    return <EditorView />;
  };

  return (
    <div className="h-screen flex flex-col bg-nk-darker">
      {/* Header with connection status and navigation - always show tabs */}
      <Header
        activeTab={activeTab}
        onTabChange={setActiveTab}
        showTabs={true}
      />

      {/* Device tab bar - visible when devices exist and on editor/librarian views */}
      {showDeviceTabBar && <DeviceTabBar />}

      {/* Main content area */}
      <main className="flex-1 overflow-hidden">
        {/* Settings and Help views take full width regardless of layout */}
        {activeTab === 'settings' ? (
          <div className="h-full overflow-auto">
            <SettingsView />
          </div>
        ) : activeTab === 'help' ? (
          <div className="h-full overflow-auto">
            <HelpView />
          </div>
        ) : useSplitLayout ? (
          // Split layout: Editor left, Librarian right
          <div className="h-full flex">
            <div className="flex-1 border-r border-nk-border overflow-auto">
              {renderEditorContent()}
            </div>
            <div className="w-96 overflow-auto">
              <LibrarianView compact />
            </div>
          </div>
        ) : (
          // Tabbed layout for Editor and Librarian
          <div className="h-full overflow-auto">
            {activeTab === 'editor' && renderEditorContent()}
            {activeTab === 'librarian' && <LibrarianView />}
          </div>
        )}
      </main>
    </div>
  );
}

export default App;
