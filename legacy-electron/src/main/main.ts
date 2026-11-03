/**
 * Electron Main Process
 *
 * Application lifecycle management for nkEditor3.
 * Handles window creation, IPC setup, and application events.
 */

import { app, BrowserWindow, ipcMain, shell } from 'electron';
import { join } from 'path';
import { registerIpcHandlers, cleanupIpcHandlers } from './ipc-handlers.js';
import { setMainWindow } from './utils/renderer-bridge.js';

/** Main application window */
let mainWindow: BrowserWindow | null = null;

/** Whether the app is running in development mode */
const isDev = process.env.NODE_ENV === 'development' || !app.isPackaged;

/**
 * Creates the main application window.
 */
function createWindow(): void {
  mainWindow = new BrowserWindow({
    width: 1400,
    height: 900,
    minWidth: 1000,
    minHeight: 700,
    title: 'nkEditor3 - nanoKONTROL2 MIDI Remapper',
    backgroundColor: '#0f0f0f',
    webPreferences: {
      preload: join(__dirname, 'preload.cjs'),
      nodeIntegration: false,
      contextIsolation: true,
      sandbox: false, // Required for easymidi native module
    },
    show: false, // Show after ready-to-show
  });

  // Update the renderer bridge with the window reference
  setMainWindow(mainWindow);

  // Show window when ready to prevent flash
  mainWindow.once('ready-to-show', () => {
    mainWindow?.show();
  });

  // Load the appropriate URL based on environment
  if (isDev) {
    // Development: load from Vite dev server
    mainWindow.loadURL('http://localhost:5173');
    // Open DevTools in development
    mainWindow.webContents.openDevTools();
  } else {
    // Production: load from built files
    mainWindow.loadFile(join(__dirname, '../dist/index.html'));
  }

  // Handle window close
  mainWindow.on('closed', () => {
    mainWindow = null;
    setMainWindow(null);
  });

  // Handle external links
  mainWindow.webContents.setWindowOpenHandler(({ url }) => {
    // Open external links in default browser
    if (url.startsWith('http://') || url.startsWith('https://')) {
      shell.openExternal(url);
    }
    return { action: 'deny' };
  });
}

/**
 * Returns the main window instance.
 */
export function getMainWindow(): BrowserWindow | null {
  return mainWindow;
}

// =============================================================================
// App Lifecycle
// =============================================================================

// Quit when all windows are closed (except on macOS)
app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') {
    app.quit();
  }
});

// On macOS, re-create window when dock icon is clicked
app.on('activate', () => {
  if (BrowserWindow.getAllWindows().length === 0) {
    createWindow();
  }
});

// App ready - create window and register IPC handlers
app.whenReady().then(() => {
  // Register IPC handlers before creating window
  registerIpcHandlers(ipcMain);

  // Create the main window
  createWindow();

  console.log('[main] nkEditor3 started');
});

// App before quit - cleanup
app.on('before-quit', () => {
  console.log('[main] Cleaning up before quit...');
  cleanupIpcHandlers(ipcMain);
});

// Handle unhandled promise rejections
process.on('unhandledRejection', (reason, promise) => {
  console.error('[main] Unhandled Rejection at:', promise, 'reason:', reason);
});

// Handle uncaught exceptions
process.on('uncaughtException', (error) => {
  console.error('[main] Uncaught Exception:', error);
  // Don't exit immediately to allow cleanup
  app.quit();
});
