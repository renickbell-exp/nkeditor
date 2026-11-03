/**
 * Config parser re-export for main process services.
 *
 * This module re-exports from the root config-parser to avoid maintaining
 * two identical copies with different import paths. The root config-parser
 * is the canonical implementation.
 */

export { parseMapping, parseMappingOrThrow, ParseError } from '../../config-parser.js';
