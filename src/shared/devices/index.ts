/**
 * Device Abstraction Layer
 *
 * Re-exports all device-related types, definitions, and registry functions.
 * Import from this barrel module rather than individual files.
 */

export type {
  MidiMessageType,
  ContinuousControlDef,
  ButtonControlDef,
  ControlDef,
  TrackLayout,
  ExtraControlsSection,
  LedProtocol,
  LedDef,
  DeviceDefinition,
} from './device-definition.js';

export { NANOKONTROL2_DEFINITION } from './nanokontrol2.js';
export { AKAI_MIDIMIX_DEFINITION } from './akai-midimix.js';

export {
  getDeviceDefinition,
  getAllDeviceDefinitions,
  identifyDevice,
  getDefaultDevice,
} from './registry.js';
