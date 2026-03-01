/**
 * IPC Handlers
 *
 * Handles all IPC messages from the renderer process.
 * Coordinates between MIDI manager, file manager, and config manager.
 */

import type { IpcMain } from 'electron';
import { IPC_INVOKE, IPC_EVENTS } from '@shared/ipc-protocol.js';
import type {
  SendCCRequest,
  ConnectRequest,
  ConnectResponse,
  GetPortsResponse,
  MidiStatusResponse,
  SavePresetRequest,
  SavePresetResponse,
  LoadPresetRequest,
  LoadPresetResponse,
  DeletePresetRequest,
  DeletePresetResponse,
  ListPresetsResponse,
  ApplyMappingRequest,
  ApplyMappingResponse,
  GetConfigResponse,
  UpdateConfigRequest,
  UpdateConfigResponse,
} from '@shared/ipc-protocol.js';
import { MidiManager } from './services/midi-manager.js';
import { FileManager } from './services/file-manager.js';
import { ConfigManager } from './services/config-manager.js';
import { sendToRenderer } from './utils/renderer-bridge.js';

// =============================================================================
// Service Instances
// =============================================================================

let midiManager: MidiManager | null = null;
let fileManager: FileManager | null = null;
let configManager: ConfigManager | null = null;

// =============================================================================
// Initialization
// =============================================================================

/**
 * Initializes all service managers.
 */
function initializeServices(): void {
  // Initialize config manager first (others may depend on config)
  configManager = new ConfigManager();

  // Initialize file manager for preset storage
  fileManager = new FileManager();

  // Initialize MIDI manager
  midiManager = new MidiManager();

  // Set up MIDI event forwarding to renderer
  setupMidiEventForwarding();
}

/**
 * Sets up MIDI event forwarding to the renderer process.
 */
function setupMidiEventForwarding(): void {
  if (!midiManager) return;

  midiManager.on('cc', (event) => {
    sendToRenderer(IPC_EVENTS.MIDI_CC, event);
  });

  midiManager.on('connected', (event) => {
    sendToRenderer(IPC_EVENTS.MIDI_CONNECTED, event);
  });

  midiManager.on('disconnected', (event) => {
    sendToRenderer(IPC_EVENTS.MIDI_DISCONNECTED, event);
  });

  midiManager.on('portsChanged', (event) => {
    sendToRenderer(IPC_EVENTS.MIDI_PORTS_CHANGED, event);
  });

  midiManager.on('error', (event) => {
    sendToRenderer(IPC_EVENTS.ERROR, event);
  });
}

// =============================================================================
// IPC Handler Registration
// =============================================================================

/**
 * Registers all IPC handlers.
 */
export function registerIpcHandlers(ipcMain: IpcMain): void {
  // Initialize services
  initializeServices();

  // MIDI handlers
  ipcMain.handle(IPC_INVOKE.MIDI_SEND, handleMidiSend);
  ipcMain.handle(IPC_INVOKE.MIDI_CONNECT, handleMidiConnect);
  ipcMain.handle(IPC_INVOKE.MIDI_DISCONNECT, handleMidiDisconnect);
  ipcMain.handle(IPC_INVOKE.MIDI_GET_PORTS, handleMidiGetPorts);
  ipcMain.handle(IPC_INVOKE.MIDI_GET_STATUS, handleMidiGetStatus);

  // Preset handlers
  ipcMain.handle(IPC_INVOKE.PRESET_LOAD, handlePresetLoad);
  ipcMain.handle(IPC_INVOKE.PRESET_SAVE, handlePresetSave);
  ipcMain.handle(IPC_INVOKE.PRESET_DELETE, handlePresetDelete);
  ipcMain.handle(IPC_INVOKE.PRESET_LIST, handlePresetList);
  ipcMain.handle(IPC_INVOKE.PRESET_APPLY, handlePresetApply);

  // Config handlers
  ipcMain.handle(IPC_INVOKE.CONFIG_GET, handleConfigGet);
  ipcMain.handle(IPC_INVOKE.CONFIG_UPDATE, handleConfigUpdate);

  console.log('[ipc-handlers] All IPC handlers registered');

  // Apply saved config settings to MIDI manager
  const config = configManager?.getConfig();
  if (config && midiManager) {
    // Initialize LED mode from config
    midiManager.setLedMode(config.ledMode);
  }

  // Auto-connect if configured
  if (config?.autoConnect) {
    console.log('[ipc-handlers] Auto-connecting to MIDI device...');
    midiManager?.connect();
  }

  // Start hotplug detection
  midiManager?.startHotplugDetection();
}

/**
 * Cleans up IPC handlers and services.
 */
export function cleanupIpcHandlers(ipcMain: IpcMain): void {
  console.log('[ipc-handlers] Cleaning up...');

  // Deregister all IPC handlers to prevent memory leaks and stale references
  ipcMain.removeHandler(IPC_INVOKE.MIDI_SEND);
  ipcMain.removeHandler(IPC_INVOKE.MIDI_CONNECT);
  ipcMain.removeHandler(IPC_INVOKE.MIDI_DISCONNECT);
  ipcMain.removeHandler(IPC_INVOKE.MIDI_GET_PORTS);
  ipcMain.removeHandler(IPC_INVOKE.MIDI_GET_STATUS);
  ipcMain.removeHandler(IPC_INVOKE.PRESET_LOAD);
  ipcMain.removeHandler(IPC_INVOKE.PRESET_SAVE);
  ipcMain.removeHandler(IPC_INVOKE.PRESET_DELETE);
  ipcMain.removeHandler(IPC_INVOKE.PRESET_LIST);
  ipcMain.removeHandler(IPC_INVOKE.PRESET_APPLY);
  ipcMain.removeHandler(IPC_INVOKE.CONFIG_GET);
  ipcMain.removeHandler(IPC_INVOKE.CONFIG_UPDATE);

  // Disconnect and cleanup MIDI
  if (midiManager) {
    midiManager.disconnect();
    midiManager = null;
  }

  // Cleanup config manager (clears interval and saves pending changes)
  if (configManager) {
    configManager.cleanup();
    configManager = null;
  }

  fileManager = null;
}

// =============================================================================
// MIDI Handlers
// =============================================================================

async function handleMidiSend(_event: Electron.IpcMainInvokeEvent, request: SendCCRequest): Promise<void> {
  if (!midiManager) {
    throw new Error('MIDI manager not initialized');
  }
  midiManager.sendCC(request.channel, request.cc, request.value);
}

async function handleMidiConnect(_event: Electron.IpcMainInvokeEvent, request?: ConnectRequest): Promise<ConnectResponse> {
  if (!midiManager) {
    return { success: false, error: 'MIDI manager not initialized' };
  }

  const success = midiManager.connect(request?.inputPort, request?.outputPort);
  const ports = midiManager.getCurrentPorts();

  return {
    success,
    inputPort: ports.input ?? undefined,
    outputPort: ports.output ?? undefined,
    error: success ? undefined : 'Failed to connect to MIDI device',
  };
}

async function handleMidiDisconnect(): Promise<void> {
  if (midiManager) {
    midiManager.disconnect();
  }
}

async function handleMidiGetPorts(): Promise<GetPortsResponse> {
  if (!midiManager) {
    return { inputs: [], outputs: [], nanoKontrol2Found: false };
  }
  return midiManager.getAvailablePorts();
}

async function handleMidiGetStatus(): Promise<MidiStatusResponse> {
  if (!midiManager) {
    return {
      connected: false,
      inputPort: null,
      outputPort: null,
      virtualPort: 'nkEditor3 Out',
      midiAvailable: false,
    };
  }
  return midiManager.getStatus();
}

// =============================================================================
// Preset Handlers
// =============================================================================

async function handlePresetLoad(_event: Electron.IpcMainInvokeEvent, request: LoadPresetRequest): Promise<LoadPresetResponse> {
  if (!fileManager) {
    return { success: false, error: 'File manager not initialized' };
  }
  return fileManager.loadPreset(request.id);
}

async function handlePresetSave(_event: Electron.IpcMainInvokeEvent, request: SavePresetRequest): Promise<SavePresetResponse> {
  if (!fileManager) {
    return { success: false, error: 'File manager not initialized' };
  }
  return fileManager.savePreset(request.preset, request.overwrite);
}

async function handlePresetDelete(_event: Electron.IpcMainInvokeEvent, request: DeletePresetRequest): Promise<DeletePresetResponse> {
  if (!fileManager) {
    return { success: false, error: 'File manager not initialized' };
  }
  return fileManager.deletePreset(request.id);
}

async function handlePresetList(): Promise<ListPresetsResponse> {
  if (!fileManager) {
    return { presets: [] };
  }
  return fileManager.listPresets();
}

async function handlePresetApply(_event: Electron.IpcMainInvokeEvent, request: ApplyMappingRequest): Promise<ApplyMappingResponse> {
  if (!midiManager) {
    return { success: false, error: 'MIDI manager not initialized' };
  }

  try {
    // MIDI manager currently only supports nK2-shaped MappingConfig
    if ('deviceId' in request.mapping) {
      return { success: false, error: 'DeviceMappingConfig is not yet supported by the MIDI manager' };
    }
    midiManager.loadMapping(request.mapping);
    return { success: true };
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    return { success: false, error: message };
  }
}

// =============================================================================
// Config Handlers
// =============================================================================

async function handleConfigGet(): Promise<GetConfigResponse> {
  if (!configManager) {
    throw new Error('Config manager not initialized');
  }
  return { config: configManager.getConfig() };
}

async function handleConfigUpdate(_event: Electron.IpcMainInvokeEvent, request: UpdateConfigRequest): Promise<UpdateConfigResponse> {
  if (!configManager) {
    return { success: false, error: 'Config manager not initialized' };
  }

  try {
    const config = configManager.update(request.updates);

    // Propagate LED mode changes to MIDI manager
    if (request.updates.ledMode !== undefined && midiManager) {
      midiManager.setLedMode(request.updates.ledMode);
    }

    return { success: true, config };
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    return { success: false, error: message };
  }
}
