/**
 * Zero-Dependency OSC (Open Sound Control 1.0 / 1.1) Binary Codec (`js/osc-codec.js`)
 *
 * Encodes and decodes standard 4-byte aligned OSC messages (`OscMessage`) and
 * bundles (`#bundle\0`) for UDP communication with the pure Rust `nkeditor`
 * (`rosc`) server (`src/osc_server.rs`).
 */

import { Buffer } from 'node:buffer';

/**
 * Pads a byte length up to the next 4-byte boundary.
 * Note: OSC strings require at least 1 NUL terminator byte before rounding to 4 bytes.
 */
export function padTo4(byteLen) {
  const remainder = byteLen % 4;
  return remainder === 0 ? byteLen : byteLen + (4 - remainder);
}

/**
 * Encodes a JavaScript string as a NUL-terminated, 4-byte aligned OSC string Buffer.
 */
export function encodeOscString(str) {
  const raw = Buffer.from(String(str), 'utf8');
  const paddedLen = padTo4(raw.length + 1);
  const out = Buffer.alloc(paddedLen, 0);
  raw.copy(out, 0);
  return out;
}

/**
 * Decodes a NUL-terminated, 4-byte aligned OSC string from `buffer` starting at `offset`.
 * Returns `{ value: string, nextOffset: number }`.
 */
export function decodeOscString(buffer, offset = 0) {
  let nulIndex = offset;
  while (nulIndex < buffer.length && buffer[nulIndex] !== 0) {
    nulIndex += 1;
  }
  const value = buffer.toString('utf8', offset, nulIndex);
  const consumed = padTo4(nulIndex - offset + 1);
  return {
    value,
    nextOffset: offset + consumed,
  };
}

/**
 * Infers an explicit OSC `{ type, value }` descriptor from a plain JS value or typed wrapper.
 */
export function normalizeOscArg(arg) {
  if (arg !== null && typeof arg === 'object' && typeof arg.type === 'string') {
    return arg;
  }
  if (typeof arg === 'string') {
    return { type: 's', value: arg };
  }
  if (typeof arg === 'boolean') {
    return { type: arg ? 'T' : 'F', value: arg };
  }
  if (typeof arg === 'number') {
    if (Number.isInteger(arg) && arg >= -2147483648 && arg <= 2147483647) {
      return { type: 'i', value: arg };
    }
    return { type: 'f', value: arg };
  }
  if (Buffer.isBuffer(arg) || arg instanceof Uint8Array) {
    return { type: 'b', value: Buffer.from(arg) };
  }
  return { type: 's', value: String(arg ?? '') };
}

/**
 * Encodes an OSC Message `{ addr: string, args?: Array }` into a UDP-ready `Buffer`.
 */
export function encodeOscMessage(addrOrMsg, maybeArgs = []) {
  const addr = typeof addrOrMsg === 'string' ? addrOrMsg : addrOrMsg.addr;
  const rawArgs =
    typeof addrOrMsg === 'string' ? maybeArgs : (addrOrMsg.args ?? []);

  if (!addr || !addr.startsWith('/')) {
    throw new Error(`OSC address must start with '/': got '${addr}'`);
  }

  const normalizedArgs = rawArgs.map(normalizeOscArg);
  const typeTags = `,${normalizedArgs.map((a) => a.type).join('')}`;

  const chunks = [encodeOscString(addr), encodeOscString(typeTags)];

  for (const arg of normalizedArgs) {
    switch (arg.type) {
      case 'i': {
        const b = Buffer.alloc(4);
        b.writeInt32BE(Math.trunc(Number(arg.value) || 0), 0);
        chunks.push(b);
        break;
      }
      case 'f': {
        const b = Buffer.alloc(4);
        b.writeFloatBE(Number(arg.value) || 0, 0);
        chunks.push(b);
        break;
      }
      case 's': {
        chunks.push(encodeOscString(arg.value));
        break;
      }
      case 'b': {
        const raw = Buffer.from(arg.value);
        const paddedLen = padTo4(raw.length);
        const b = Buffer.alloc(4 + paddedLen, 0);
        b.writeInt32BE(raw.length, 0);
        raw.copy(b, 4);
        chunks.push(b);
        break;
      }
      case 'T':
      case 'F':
      case 'N':
        // No payload bytes for boolean/nil tags
        break;
      default:
        throw new Error(`Unsupported OSC argument type tag: '${arg.type}'`);
    }
  }

  return Buffer.concat(chunks);
}

/**
 * Decodes a single OSC Message from a `Buffer`.
 * Returns `{ addr: string, args: Array<string|number|boolean|Buffer>, typedArgs: Array<{type, value}> }`.
 */
export function decodeOscMessage(buffer, startOffset = 0) {
  const { value: addr, nextOffset: afterAddr } = decodeOscString(
    buffer,
    startOffset
  );

  if (afterAddr >= buffer.length) {
    return { addr, args: [], typedArgs: [] };
  }

  const { value: typeTagStr, nextOffset: afterTags } = decodeOscString(
    buffer,
    afterAddr
  );

  const tags = typeTagStr.startsWith(',')
    ? typeTagStr.slice(1)
    : typeTagStr;

  const args = [];
  const typedArgs = [];
  let offset = afterTags;

  for (let i = 0; i < tags.length; i += 1) {
    const tag = tags[i];
    switch (tag) {
      case 'i': {
        const val = buffer.readInt32BE(offset);
        offset += 4;
        args.push(val);
        typedArgs.push({ type: 'i', value: val });
        break;
      }
      case 'f': {
        const val = buffer.readFloatBE(offset);
        offset += 4;
        args.push(val);
        typedArgs.push({ type: 'f', value: val });
        break;
      }
      case 's': {
        const { value: strVal, nextOffset } = decodeOscString(buffer, offset);
        offset = nextOffset;
        args.push(strVal);
        typedArgs.push({ type: 's', value: strVal });
        break;
      }
      case 'b': {
        const blobLen = buffer.readInt32BE(offset);
        offset += 4;
        const blob = buffer.subarray(offset, offset + blobLen);
        offset += padTo4(blobLen);
        args.push(blob);
        typedArgs.push({ type: 'b', value: blob });
        break;
      }
      case 'T': {
        args.push(true);
        typedArgs.push({ type: 'T', value: true });
        break;
      }
      case 'F': {
        args.push(false);
        typedArgs.push({ type: 'F', value: false });
        break;
      }
      case 'N': {
        args.push(null);
        typedArgs.push({ type: 'N', value: null });
        break;
      }
      default:
        break;
    }
  }

  return { addr, args, typedArgs };
}

/**
 * Encodes an OSC Bundle (`#bundle\0`) containing multiple OSC messages.
 */
export function encodeOscBundle(messages, timetag = 1n) {
  const header = encodeOscString('#bundle'); // 8 bytes
  const timeBuf = Buffer.alloc(8);
  timeBuf.writeBigUInt64BE(BigInt(timetag), 0);

  const elements = [header, timeBuf];
  for (const msg of messages) {
    const encodedMsg = Buffer.isBuffer(msg)
      ? msg
      : encodeOscMessage(msg.addr, msg.args);
    const lenBuf = Buffer.alloc(4);
    lenBuf.writeInt32BE(encodedMsg.length, 0);
    elements.push(lenBuf, encodedMsg);
  }
  return Buffer.concat(elements);
}

/**
 * Decodes either a single OSC Message or a flat array of OSC Messages from an OSC Bundle.
 */
export function decodeOscPacket(buffer) {
  if (buffer.length >= 8 && buffer.toString('utf8', 0, 7) === '#bundle') {
    const messages = [];
    let offset = 16; // 8 bytes "#bundle\0" + 8 bytes timetag
    while (offset + 4 <= buffer.length) {
      const elemSize = buffer.readInt32BE(offset);
      offset += 4;
      if (elemSize <= 0 || offset + elemSize > buffer.length) {
        break;
      }
      const subBuf = buffer.subarray(offset, offset + elemSize);
      messages.push(...decodeOscPacket(subBuf));
      offset += elemSize;
    }
    return messages;
  }
  return [decodeOscMessage(buffer, 0)];
}
