/**
 * Header Component
 *
 * Application header with connection status and tab navigation.
 */

import React from 'react';
import { useConnectionStore } from '../stores/connection';
import { useDevicesStore } from '../stores/devices';
import { DeviceSelector } from './DeviceSelector';

interface HeaderProps {
  activeTab: 'editor' | 'librarian' | 'settings' | 'help';
  onTabChange: (tab: 'editor' | 'librarian' | 'settings' | 'help') => void;
  showTabs: boolean;
}

export function Header({ activeTab, onTabChange, showTabs }: HeaderProps): React.ReactElement {
  const { connected, inputPort, connecting } = useConnectionStore();
  const deviceCount = useDevicesStore((state) => state.devices.size);

  return (
    <header className="flex items-center justify-between px-4 py-2 bg-nk-dark border-b border-nk-border">
      {/* Logo and title */}
      <div className="flex items-center gap-3">
        <div className="w-8 h-8 rounded bg-nk-accent flex items-center justify-center font-bold text-white">
          nK
        </div>
        <div>
          <h1 className="text-lg font-semibold text-white">nkEditor3</h1>
          <p className="text-xs text-gray-500">MIDI CC Remapper</p>
        </div>
      </div>

      {/* Tab navigation (only shown in narrow layout) */}
      {showTabs && (
        <nav className="flex gap-1">
          <TabButton
            active={activeTab === 'editor'}
            onClick={() => onTabChange('editor')}
          >
            Editor
          </TabButton>
          <TabButton
            active={activeTab === 'librarian'}
            onClick={() => onTabChange('librarian')}
          >
            Librarian
          </TabButton>
          <TabButton
            active={activeTab === 'settings'}
            onClick={() => onTabChange('settings')}
          >
            Settings
          </TabButton>
          <TabButton
            active={activeTab === 'help'}
            onClick={() => onTabChange('help')}
          >
            Help
          </TabButton>
        </nav>
      )}

      {/* Device selector and connection status */}
      <div className="flex items-center gap-3">
        <DeviceSelector />
        {deviceCount === 0 && (
          <ConnectionStatus
            connected={connected}
            connecting={connecting}
            portName={inputPort}
          />
        )}
      </div>
    </header>
  );
}

// =============================================================================
// Sub-components
// =============================================================================

interface TabButtonProps {
  active: boolean;
  onClick: () => void;
  children: React.ReactNode;
}

function TabButton({ active, onClick, children }: TabButtonProps): React.ReactElement {
  return (
    <button
      className={`px-4 py-1.5 rounded text-sm font-medium transition-colors ${
        active
          ? 'bg-nk-accent text-white'
          : 'text-gray-400 hover:text-white hover:bg-nk-light'
      }`}
      onClick={onClick}
    >
      {children}
    </button>
  );
}

interface ConnectionStatusProps {
  connected: boolean;
  connecting: boolean;
  portName: string | null;
}

function ConnectionStatus({ connected, connecting, portName }: ConnectionStatusProps): React.ReactElement {
  if (connecting) {
    return (
      <div className="status-badge status-connecting">
        <span className="w-2 h-2 rounded-full bg-yellow-400 animate-pulse" />
        <span>Connecting...</span>
      </div>
    );
  }

  if (connected) {
    return (
      <div className="status-badge status-connected">
        <span className="w-2 h-2 rounded-full bg-green-400" />
        <span title={portName ?? undefined}>Connected</span>
      </div>
    );
  }

  return (
    <div className="status-badge status-disconnected">
      <span className="w-2 h-2 rounded-full bg-red-400" />
      <span>Disconnected</span>
    </div>
  );
}
