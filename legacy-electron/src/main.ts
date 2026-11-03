/**
 * nkEditor3 CLI Entry Point
 *
 * Command-line interface for the Korg nanoKONTROL2 MIDI CC remapper.
 * Handles argument parsing, configuration loading, and orchestrates
 * the MIDI handler, mapping engine, and LED controller.
 *
 * Usage:
 *   nkeditor3                          Run with default mapping
 *   nkeditor3 -m mappings/setup.txt    Specify mapping file
 *   nkeditor3 --list-ports             List available MIDI ports
 *   nkeditor3 --validate file.txt      Validate mapping file
 *   nkeditor3 -v, --verbose            Verbose output mode
 *   nkeditor3 --debug                  Debug mode (extra logging)
 */

import { Command } from 'commander';
import fs from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

import { parseMapping } from './config-parser.js';
import { MidiHandler } from './midi-handler.js';
import { MappingEngine, type ProcessedMessage, type ButtonStateChangeEvent } from './mapping-engine.js';
import { LedController } from './led-controller.js';
import { getAvailablePorts, formatPortsForDisplay, isMidiAvailable } from './midi-discovery.js';
import type { CCMessage, MappingConfig, ConnectedEvent, DisconnectedEvent, ErrorEvent } from './types.js';

// =============================================================================
// Constants
// =============================================================================

const VERSION = '1.0.0';
const APP_NAME = 'nkEditor3';
const DEFAULT_MAPPING_FILENAME = 'default.txt';

/**
 * Represents the MIDI subsystem status.
 * - 'available': MIDI subsystem is working, can enumerate ports
 * - 'unavailable': MIDI subsystem failed (e.g., ALSA not available, no permissions)
 */
type MidiSubsystemStatus = 'available' | 'unavailable';

/**
 * Tracks whether we have already logged a particular error message.
 * Prevents spamming the same error repeatedly during hotplug polling.
 */
const loggedErrors = new Set<string>();

/**
 * Tracks whether the MIDI subsystem unavailability has been logged.
 * Ensures we only show the "running in offline mode" message once.
 */
let midiUnavailableLogged = false;

// =============================================================================
// CLI Options Interface
// =============================================================================

interface CliOptions {
  mapping?: string;
  listPorts?: boolean;
  input?: string;
  output?: string;
  verbose?: boolean;
  debug?: boolean;
  validate?: string;
}

// =============================================================================
// Logging Utilities
// =============================================================================

let verboseMode = false;
let debugMode = false;

/**
 * Logs an informational message.
 */
function logInfo(message: string): void {
  console.log(`[INFO] ${message}`);
}

/**
 * Logs an error message.
 */
function logError(message: string): void {
  console.error(`[ERROR] ${message}`);
}

/**
 * Logs a warning message.
 */
function logWarn(message: string): void {
  console.warn(`[WARN] ${message}`);
}


/**
 * Logs a debug message (only if debug mode is enabled).
 */
function logDebug(message: string): void {
  if (debugMode) {
    console.log(`[DEBUG] ${message}`);
  }
}

/**
 * Logs an error message only once (prevents spam during hotplug polling).
 * Uses a hash of the message to track which errors have already been logged.
 *
 * @param message - Error message to log
 * @param resetKey - Optional key to reset after (e.g., on successful connection)
 */
function logErrorOnce(message: string): void {
  if (loggedErrors.has(message)) {
    return;
  }
  loggedErrors.add(message);
  logError(message);
}

/**
 * Clears the logged errors set, allowing errors to be logged again.
 * Called when the device successfully connects to reset error tracking.
 */
function clearLoggedErrors(): void {
  loggedErrors.clear();
}

/**
 * Logs a CC message in verbose format.
 */
function logCCMessage(msg: ProcessedMessage): void {
  if (!verboseMode && !debugMode) return;

  const stateStr = msg.isButton && msg.buttonState !== undefined
    ? (msg.buttonState ? 'ON' : 'OFF')
    : '';

  const labelStr = msg.label || msg.controlType;
  const behaviorHint = msg.isButton ? ' (toggle)' : '';

  if (msg.isButton) {
    console.log(`[CC] ${labelStr}${behaviorHint}: ${stateStr} -> ch${msg.channel} cc${msg.outputCC} val:${msg.value}`);
  } else {
    console.log(`[CC] ${labelStr}: ${msg.value} -> ch${msg.channel} cc${msg.outputCC} val:${msg.value}`);
  }
}

/**
 * Logs an LED state change in verbose format.
 */
function logLedChange(_cc: number, isOn: boolean, controlType: string): void {
  if (!verboseMode && !debugMode) return;
  console.log(`[LED] ${controlType}: ${isOn ? 'ON' : 'OFF'}`);
}

// =============================================================================
// MIDI Subsystem Detection
// =============================================================================

/**
 * Checks whether the MIDI subsystem is available on this system.
 *
 * Delegates to the midi-discovery module's isMidiAvailable() function,
 * which handles caching and error deduplication.
 *
 * This is different from "device not found" - if the subsystem is available
 * but no nanoKONTROL2 is connected, that's a normal hotplug scenario.
 * If the subsystem itself is unavailable, we should run in offline mode.
 *
 * IMPORTANT: This function only checks if the MIDI subsystem can be accessed.
 * It does NOT enumerate ports, as port enumeration is done later by MidiHandler
 * and doing it here would create redundant ALSA client initialization.
 *
 * @returns Object with status
 */
function checkMidiSubsystem(): { status: MidiSubsystemStatus } {
  // Use the discovery module's availability check which handles caching
  // and prevents repeated initialization attempts.
  // NOTE: We intentionally do NOT call getAvailablePorts() here to avoid
  // redundant ALSA client creation. The isMidiAvailable() check uses a
  // single lightweight call to easymidi.getInputs() which is sufficient
  // to verify the subsystem is accessible.
  const available = isMidiAvailable();

  if (available) {
    logDebug('MIDI subsystem check: subsystem accessible');
    return { status: 'available' };
  }

  return { status: 'unavailable' };
}

/**
 * Logs the offline mode banner when MIDI subsystem is unavailable.
 * Only logs once per session to avoid spam.
 */
function logOfflineModeBanner(): void {
  if (midiUnavailableLogged) {
    return;
  }
  midiUnavailableLogged = true;

  console.log('');
  console.log('================================================================================');
  console.log('  RUNNING IN OFFLINE MODE - MIDI subsystem unavailable');
  console.log('================================================================================');
  console.log('');
  console.log('  The MIDI subsystem could not be initialized. This typically means:');
  console.log('  - ALSA sequencer is not available (container/VM without audio)');
  console.log('  - Insufficient permissions to access /dev/snd/seq');
  console.log('  - No sound card or MIDI interface present');
  console.log('');
  console.log('  You can still:');
  console.log('  - Validate mapping files with --validate');
  console.log('  - View the loaded configuration');
  console.log('');
  console.log('  The application will remain running. If MIDI becomes available,');
  console.log('  restart the application to connect.');
  console.log('');
  console.log('================================================================================');
  console.log('');
}

// =============================================================================
// Path Resolution
// =============================================================================

/**
 * Gets the directory where the script is located.
 * Handles both development (ts-node) and production (compiled) scenarios.
 */
function getScriptDir(): string {
  const currentFileUrl = import.meta.url;
  const currentFilePath = fileURLToPath(currentFileUrl);
  return path.dirname(currentFilePath);
}

/**
 * Resolves the path to the default mapping file.
 */
function getDefaultMappingPath(): string {
  const scriptDir = getScriptDir();
  // In production, mappings folder is sibling to dist
  // In development, it's at project root
  const possiblePaths = [
    path.join(scriptDir, '..', 'mappings', DEFAULT_MAPPING_FILENAME),
    path.join(scriptDir, '..', '..', 'mappings', DEFAULT_MAPPING_FILENAME),
    path.join(process.cwd(), 'mappings', DEFAULT_MAPPING_FILENAME),
  ];

  for (const mappingPath of possiblePaths) {
    if (fs.existsSync(mappingPath)) {
      return path.resolve(mappingPath);
    }
  }

  // Return the most likely path even if it doesn't exist
  // Error will be caught when trying to read
  return path.join(process.cwd(), 'mappings', DEFAULT_MAPPING_FILENAME);
}

/**
 * Resolves a mapping file path, handling relative and absolute paths.
 */
function resolveMappingPath(mappingPath: string): string {
  if (path.isAbsolute(mappingPath)) {
    return mappingPath;
  }
  return path.resolve(process.cwd(), mappingPath);
}

// =============================================================================
// Mapping File Loading
// =============================================================================

/**
 * Loads and parses a mapping file.
 *
 * @param filePath - Path to the mapping file
 * @returns Parsed MappingConfig or null on failure
 */
function loadMappingFile(filePath: string): MappingConfig | null {
  const resolvedPath = resolveMappingPath(filePath);

  logInfo(`Loading mapping file: ${resolvedPath}`);

  if (!fs.existsSync(resolvedPath)) {
    logError(`Mapping file not found: ${resolvedPath}`);
    return null;
  }

  let content: string;
  try {
    content = fs.readFileSync(resolvedPath, 'utf-8');
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    logError(`Failed to read mapping file: ${message}`);
    return null;
  }

  const result = parseMapping(content);

  if (!result.success) {
    logError(`Failed to parse mapping file: ${result.error}`);
    return null;
  }

  // Count total mappings: 8 tracks * 5 controls + 11 transport = 51
  const trackMappings = result.config.tracks.length * 5;
  const transportMappings = 11;
  const totalMappings = trackMappings + transportMappings;

  logInfo(`Parsed ${totalMappings} CC mappings`);

  return result.config;
}

// =============================================================================
// Command Handlers
// =============================================================================

/**
 * Handles the --list-ports command.
 * Lists all available MIDI input and output ports.
 */
function handleListPorts(): void {
  console.log(`${APP_NAME} v${VERSION} - Korg nanoKONTROL2 MIDI Remapper\n`);

  const ports = getAvailablePorts();
  console.log(formatPortsForDisplay(ports));
}

/**
 * Handles the --validate command.
 * Validates a mapping file without starting the remapper.
 *
 * @param filePath - Path to the mapping file to validate
 * @returns Exit code (0 for success, 1 for failure)
 */
function handleValidate(filePath: string): number {
  console.log(`${APP_NAME} v${VERSION} - Mapping File Validator\n`);

  const config = loadMappingFile(filePath);

  if (config === null) {
    console.log('\nValidation FAILED');
    return 1;
  }

  console.log('\nValidation PASSED');
  console.log(`  - 8 track sections defined`);
  console.log(`  - Transport section defined`);
  console.log(`  - All required controls present`);

  return 0;
}

/**
 * Main application logic.
 * Sets up MIDI connections, mapping engine, and LED controller.
 *
 * The application operates in two modes:
 * 1. Normal mode: MIDI subsystem available, device may or may not be connected
 * 2. Offline mode: MIDI subsystem unavailable, app stays running for validation
 *
 * @param options - CLI options
 * @returns Exit code (0 for success, non-zero for failure)
 */
async function runApplication(options: CliOptions): Promise<number> {
  console.log(`${APP_NAME} v${VERSION} - Korg nanoKONTROL2 MIDI Remapper\n`);

  // Set logging modes
  verboseMode = options.verbose ?? false;
  debugMode = options.debug ?? false;

  if (debugMode) {
    logDebug('Debug mode enabled');
  }

  // Load mapping file first - this works regardless of MIDI availability
  const mappingPath = options.mapping ?? getDefaultMappingPath();
  const config = loadMappingFile(mappingPath);

  if (config === null) {
    return 1;
  }

  // Check MIDI subsystem availability before attempting any MIDI operations
  logInfo('Checking MIDI subsystem...');
  const midiSubsystem = checkMidiSubsystem();

  if (midiSubsystem.status === 'unavailable') {
    // MIDI subsystem is completely unavailable - run in offline mode
    logOfflineModeBanner();
    return runOfflineMode(options);
  }

  // MIDI subsystem is available - proceed with normal startup
  logInfo('MIDI subsystem available.');
  return runWithMidi(options, config);
}

/**
 * Runs the application in offline mode when MIDI is unavailable.
 * Keeps the process alive and allows graceful shutdown.
 *
 * @param _options - CLI options (unused in offline mode but kept for consistency)
 * @returns Never resolves - runs until interrupted
 */
async function runOfflineMode(_options: CliOptions): Promise<number> {
  logInfo('Press Ctrl+C to exit.\n');

  // Set up graceful shutdown
  const shutdown = createShutdownHandler(null, null);
  process.on('SIGINT', shutdown);
  process.on('SIGTERM', shutdown);

  // Keep the process alive with a simple interval
  // Using setInterval ensures the event loop stays active
  const keepAliveInterval = setInterval(() => {
    // Intentionally empty - just keeping the process alive
  }, 60000);

  // Allow the process to exit if this is the only thing keeping it alive
  keepAliveInterval.unref();

  // Actually, we need to ref() it to keep the process running
  keepAliveInterval.ref();

  // Never resolves - application runs until interrupted
  return new Promise(() => {});
}

/**
 * Runs the application with MIDI support enabled.
 *
 * @param options - CLI options
 * @param config - Parsed mapping configuration
 * @returns Never resolves - runs until interrupted
 */
async function runWithMidi(options: CliOptions, config: MappingConfig): Promise<number> {
  logInfo('Scanning for MIDI ports...');

  // Build handler options, only including ports if explicitly specified
  const handlerOptions: { inputPort?: string; outputPort?: string; virtualPortName?: string } = {};
  if (options.input !== undefined) {
    handlerOptions.inputPort = options.input;
  }
  if (options.output !== undefined) {
    handlerOptions.outputPort = options.output;
  }

  const midiHandler = new MidiHandler(handlerOptions);

  // Create mapping engine
  const mappingEngine = new MappingEngine(config);

  // Create LED controller
  const ledController = new LedController(midiHandler);

  // Set up event handlers (with error deduplication)
  setupEventHandlers(midiHandler, mappingEngine, ledController);

  // Try to connect (may fail if device not present)
  // The connect() method handles auto-detection internally, so we don't
  // call findNanoKontrol2() separately - that would create redundant ALSA clients
  const connected = midiHandler.connect(options.input, options.output);

  if (connected) {
    const ports = midiHandler.getCurrentPorts();
    logInfo(`Connected to: ${ports.input}`);
    logInfo(`Created virtual output: "${midiHandler.getVirtualPortName()}"`);
    logInfo('Remapping active.');

    // Sync LEDs on initial connection
    ledController.syncFromEngine(mappingEngine);

    // Clear any logged errors since we successfully connected
    clearLoggedErrors();
  } else {
    // Connection failed - provide guidance based on whether ports were specified
    if (options.input && options.output) {
      logWarn(`Could not connect to specified ports.`);
      logInfo(`Input: ${options.input}`);
      logInfo(`Output: ${options.output}`);
    } else {
      logWarn('nanoKONTROL2 not detected.');
      logInfo('Connect your nanoKONTROL2 and ensure it is in CC mode.');
      logInfo('(Hold SET MARKER + CYCLE to enter CC mode)');
    }
    logInfo('Waiting for device connection...');
  }

  // Start hotplug detection (will silently retry if device not connected)
  midiHandler.startHotplugDetection();

  logInfo('Press Ctrl+C to exit.\n');

  // Set up graceful shutdown
  const shutdown = createShutdownHandler(midiHandler, ledController);
  process.on('SIGINT', shutdown);
  process.on('SIGTERM', shutdown);

  // Keep the process running
  // The event loop will be kept alive by the MIDI input listener
  // and the hotplug detection interval
  return new Promise(() => {
    // Never resolves - application runs until interrupted
  });
}

/**
 * Creates a shutdown handler function for graceful process termination.
 *
 * @param midiHandler - MidiHandler instance to disconnect, or null for offline mode
 * @param ledController - LedController instance for turning off LEDs, or null for offline mode
 * @returns Shutdown handler function
 */
function createShutdownHandler(
  midiHandler: MidiHandler | null,
  ledController: LedController | null
): () => void {
  return (): void => {
    console.log('\n');
    logInfo('Shutting down...');

    // Turn off all LEDs if controller is available
    if (ledController) {
      ledController.allOff();
    }

    // Stop hotplug detection and disconnect if handler is available
    if (midiHandler) {
      midiHandler.disconnect();
    }

    logInfo('Goodbye.');
    process.exit(0);
  };
}

/**
 * Sets up event handlers to connect the components.
 *
 * Includes error deduplication to prevent spam during hotplug polling.
 * Errors are only logged once until the device successfully connects,
 * at which point the error log is cleared.
 */
function setupEventHandlers(
  midiHandler: MidiHandler,
  mappingEngine: MappingEngine,
  ledController: LedController
): void {
  // Handle incoming CC from hardware
  midiHandler.on('cc', (msg: CCMessage) => {
    logDebug(`Received CC: ch${msg.channel + 1} cc${msg.controller} val${msg.value}`);

    const processed = mappingEngine.processCC(msg.channel, msg.controller, msg.value);

    if (processed) {
      // Send remapped CC to virtual output
      // Convert channel from 1-indexed (display) to 0-indexed (MIDI)
      midiHandler.sendCC(processed.channel - 1, processed.outputCC, processed.value);
      logCCMessage(processed);
    }
  });

  // Handle button state changes (for LED updates)
  mappingEngine.on('buttonStateChanged', (event: ButtonStateChangeEvent) => {
    ledController.setLed(event.cc, event.isOn);
    logLedChange(event.cc, event.isOn, event.controlType);
  });

  // Handle device connection
  midiHandler.on('connected', (event: ConnectedEvent) => {
    // Clear error log on successful connection to allow fresh error reporting
    clearLoggedErrors();

    logInfo(`Device connected: ${event.inputPort}`);
    logInfo(`Created virtual output: "${midiHandler.getVirtualPortName()}"`);

    // Sync LEDs on reconnection
    ledController.syncFromEngine(mappingEngine);
  });

  // Handle device disconnection
  midiHandler.on('disconnected', (event: DisconnectedEvent) => {
    logWarn(`Device disconnected: ${event.reason}`);
    logInfo('Waiting for device reconnection...');
  });

  // Handle errors with deduplication to prevent spam during hotplug polling
  midiHandler.on('error', (event: ErrorEvent) => {
    // Use logErrorOnce to prevent repeated logging of the same error
    // This is especially important during hotplug detection which polls
    // every 2 seconds and would otherwise spam "device not found" errors
    logErrorOnce(event.message);
  });
}

// =============================================================================
// CLI Setup
// =============================================================================

/**
 * Sets up and runs the CLI.
 */
function main(): void {
  const program = new Command();

  program
    .name('nkeditor3')
    .description('MIDI CC remapper for Korg nanoKONTROL2')
    .version(VERSION, '-V, --version', 'Output the version number');

  program
    .option('-m, --mapping <file>', 'Mapping file to use (default: mappings/default.txt)')
    .option('--list-ports', 'List available MIDI ports and exit')
    .option('--input <port>', 'Hardware input port name (auto-detect if not specified)')
    .option('--output <port>', 'Hardware output port name (auto-detect if not specified)')
    .option('-v, --verbose', 'Enable verbose output (show CC messages)')
    .option('--debug', 'Enable debug output (extra logging)')
    .option('--validate <file>', 'Validate a mapping file without running');

  program.parse(process.argv);

  const options = program.opts<CliOptions>();

  // Handle --list-ports
  if (options.listPorts) {
    handleListPorts();
    process.exit(0);
  }

  // Handle --validate
  if (options.validate) {
    const exitCode = handleValidate(options.validate);
    process.exit(exitCode);
  }

  // Run the main application
  runApplication(options)
    .then((exitCode) => {
      process.exit(exitCode);
    })
    .catch((err) => {
      logError(err instanceof Error ? err.message : String(err));
      process.exit(1);
    });
}

// Run the CLI
main();
