/**
 * Preload Script
 *
 * Securely exposes IPC communication to the renderer process.
 * Uses contextBridge to provide a safe API without exposing Node.js.
 */

import { contextBridge, ipcRenderer } from 'electron';
import { IPC_EVENTS, IPC_INVOKE } from '@shared/ipc-protocol.js';
import type {
  MidiCCEvent,
  MidiConnectedEvent,
  MidiDisconnectedEvent,
  MidiPortsChangedEvent,
  ErrorEvent,
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

/**
 * API exposed to renderer process via window.electronAPI
 */
const electronAPI = {
  // ==========================================================================
  // MIDI Operations
  // ==========================================================================

  /**
   * Sends a CC message to the virtual MIDI output.
   */
  sendCC: (request: SendCCRequest): Promise<void> => {
    return ipcRenderer.invoke(IPC_INVOKE.MIDI_SEND, request);
  },

  /**
   * Connects to MIDI device.
   */
  connect: (request?: ConnectRequest): Promise<ConnectResponse> => {
    return ipcRenderer.invoke(IPC_INVOKE.MIDI_CONNECT, request);
  },

  /**
   * Disconnects from MIDI device.
   */
  disconnect: (): Promise<void> => {
    return ipcRenderer.invoke(IPC_INVOKE.MIDI_DISCONNECT);
  },

  /**
   * Gets available MIDI ports.
   */
  getPorts: (): Promise<GetPortsResponse> => {
    return ipcRenderer.invoke(IPC_INVOKE.MIDI_GET_PORTS);
  },

  /**
   * Gets MIDI connection status.
   */
  getStatus: (): Promise<MidiStatusResponse> => {
    return ipcRenderer.invoke(IPC_INVOKE.MIDI_GET_STATUS);
  },

  // ==========================================================================
  // Preset Operations
  // ==========================================================================

  /**
   * Loads a preset by ID.
   */
  loadPreset: (request: LoadPresetRequest): Promise<LoadPresetResponse> => {
    return ipcRenderer.invoke(IPC_INVOKE.PRESET_LOAD, request);
  },

  /**
   * Saves a preset.
   */
  savePreset: (request: SavePresetRequest): Promise<SavePresetResponse> => {
    return ipcRenderer.invoke(IPC_INVOKE.PRESET_SAVE, request);
  },

  /**
   * Deletes a preset.
   */
  deletePreset: (request: DeletePresetRequest): Promise<DeletePresetResponse> => {
    return ipcRenderer.invoke(IPC_INVOKE.PRESET_DELETE, request);
  },

  /**
   * Lists all presets.
   */
  listPresets: (): Promise<ListPresetsResponse> => {
    return ipcRenderer.invoke(IPC_INVOKE.PRESET_LIST);
  },

  /**
   * Applies a mapping configuration to the MIDI manager.
   */
  applyMapping: (request: ApplyMappingRequest): Promise<ApplyMappingResponse> => {
    return ipcRenderer.invoke(IPC_INVOKE.PRESET_APPLY, request);
  },

  // ==========================================================================
  // Configuration Operations
  // ==========================================================================

  /**
   * Gets application configuration.
   */
  getConfig: (): Promise<GetConfigResponse> => {
    return ipcRenderer.invoke(IPC_INVOKE.CONFIG_GET);
  },

  /**
   * Updates application configuration.
   */
  updateConfig: (request: UpdateConfigRequest): Promise<UpdateConfigResponse> => {
    return ipcRenderer.invoke(IPC_INVOKE.CONFIG_UPDATE, request);
  },

  // ==========================================================================
  // Event Listeners
  // ==========================================================================

  /**
   * Subscribes to MIDI CC events.
   */
  onMidiCC: (callback: (event: MidiCCEvent) => void): (() => void) => {
    const listener = (_event: Electron.IpcRendererEvent, data: MidiCCEvent) => callback(data);
    ipcRenderer.on(IPC_EVENTS.MIDI_CC, listener);
    return () => ipcRenderer.removeListener(IPC_EVENTS.MIDI_CC, listener);
  },

  /**
   * Subscribes to MIDI connected events.
   */
  onMidiConnected: (callback: (event: MidiConnectedEvent) => void): (() => void) => {
    const listener = (_event: Electron.IpcRendererEvent, data: MidiConnectedEvent) => callback(data);
    ipcRenderer.on(IPC_EVENTS.MIDI_CONNECTED, listener);
    return () => ipcRenderer.removeListener(IPC_EVENTS.MIDI_CONNECTED, listener);
  },

  /**
   * Subscribes to MIDI disconnected events.
   */
  onMidiDisconnected: (callback: (event: MidiDisconnectedEvent) => void): (() => void) => {
    const listener = (_event: Electron.IpcRendererEvent, data: MidiDisconnectedEvent) => callback(data);
    ipcRenderer.on(IPC_EVENTS.MIDI_DISCONNECTED, listener);
    return () => ipcRenderer.removeListener(IPC_EVENTS.MIDI_DISCONNECTED, listener);
  },

  /**
   * Subscribes to MIDI ports changed events.
   */
  onMidiPortsChanged: (callback: (event: MidiPortsChangedEvent) => void): (() => void) => {
    const listener = (_event: Electron.IpcRendererEvent, data: MidiPortsChangedEvent) => callback(data);
    ipcRenderer.on(IPC_EVENTS.MIDI_PORTS_CHANGED, listener);
    return () => ipcRenderer.removeListener(IPC_EVENTS.MIDI_PORTS_CHANGED, listener);
  },

  /**
   * Subscribes to error events.
   */
  onError: (callback: (event: ErrorEvent) => void): (() => void) => {
    const listener = (_event: Electron.IpcRendererEvent, data: ErrorEvent) => callback(data);
    ipcRenderer.on(IPC_EVENTS.ERROR, listener);
    return () => ipcRenderer.removeListener(IPC_EVENTS.ERROR, listener);
  },
};

// Expose the API to the renderer process
contextBridge.exposeInMainWorld('electronAPI', electronAPI);

// Type declaration for the exposed API
export type ElectronAPI = typeof electronAPI;
