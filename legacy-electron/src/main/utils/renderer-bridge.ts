/**
 * Renderer Bridge
 *
 * Provides a centralized mechanism for sending messages to the renderer process.
 * This module breaks the circular dependency between main.ts and ipc-handlers.ts
 * by providing a setter for the main window reference and a send function that
 * can be imported independently.
 */

import type { BrowserWindow } from 'electron';

/** Reference to the main application window */
let mainWindow: BrowserWindow | null = null;

/**
 * Sets the main window reference.
 * Should be called from main.ts after window creation and on window close.
 *
 * @param window - The main BrowserWindow instance, or null when the window is destroyed
 */
export function setMainWindow(window: BrowserWindow | null): void {
  mainWindow = window;
}

/**
 * Sends a message to the renderer process via the main window's webContents.
 * Silently does nothing if the window is not available or has been destroyed.
 *
 * @param channel - The IPC channel name to send the message on
 * @param args - Arguments to pass with the message
 */
export function sendToRenderer(channel: string, ...args: unknown[]): void {
  if (mainWindow && !mainWindow.isDestroyed()) {
    mainWindow.webContents.send(channel, ...args);
  }
}
