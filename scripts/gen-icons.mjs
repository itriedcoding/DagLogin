// Generates the Prism icon set (PNG + ICO) with no external dependencies.
// Run with: node scripts/gen-icons.mjs
import { deflateSync } from "node:zlib";
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const outDir = join(here, "..", "src-tauri", "icons");
mkdirSync(outDir, { recursive: true });

const ACCENT = [79, 140, 255];
const ACCENT_2 = [56, 214, 255];
const WHITE = [244, 248, 255];

const clamp = (v, min, max) => Math.min(max, Math.max(min, v));

function distToSegment(px, py, ax, ay, bx, by) {
  const abx = bx - ax;
  const aby = by - ay;
  const apx = px - ax;
  const apy = py - ay;
  const lengthSq = abx * abx + aby * aby || 1;
  const t = clamp((apx * abx + apy * aby) / lengthSq, 0, 1);
  const dx = px - (ax + abx * t);
  const dy = py - (ay + aby * t);
  return Math.hypot(dx, dy);
}

function blend(target, index, color, alpha) {
  if (alpha <= 0) return;
  const a = clamp(alpha, 0, 1);
  target[index] = Math.round(target[index] * (1 - a) + color[0] * a);
  target[index + 1] = Math.round(target[index + 1] * (1 - a) + color[1] * a);
  target[index + 2] = Math.round(target[index + 2] * (1 - a) + color[2] * a);
  target[index + 3] = Math.max(target[index + 3], Math.round(255 * a));
}

function render(size) {
  const data = new Uint8Array(size * size * 4);
  const s = size;
  const radius = s * 0.22;
  const inset = s * 0.02;

  // Rounded square background with a vertical gradient.
  for (let y = 0; y < s; y++) {
    for (let x = 0; x < s; x++) {
      const dx = Math.max(inset + radius - x, x - (s - inset - radius), 0);
      const dy = Math.max(inset + radius - y, y - (s - inset - radius), 0);
      const outside = Math.hypot(dx, dy) - radius + inset;
      const alpha = clamp(0.5 - outside, 0, 1);
      if (alpha <= 0) continue;
      const t = y / s;
      const color = [
        Math.round(13 + 10 * t),
        Math.round(17 + 14 * t),
        Math.round(26 + 20 * t),
      ];
      blend(data, (y * s + x) * 4, color, alpha);
    }
  }

  // Prism: an upward triangle outline.
  const apex = [s * 0.5, s * 0.2];
  const left = [s * 0.24, s * 0.76];
  const right = [s * 0.76, s * 0.76];
  const stroke = Math.max(1.2, s * 0.045);

  // An incoming white beam that splits into a spectrum.
  const beamY = s * 0.56;
  const spectrum = [
    [244, 248, 255],
    [120, 200, 255],
    [79, 140, 255],
    [150, 120, 255],
  ];

  for (let y = 0; y < s; y++) {
    for (let x = 0; x < s; x++) {
      const index = (y * s + x) * 4;
      if (data[index + 3] === 0) continue;

      // Beam entering the prism.
      const beam = distToSegment(x, y, s * 0.12, beamY, s * 0.44, beamY);
      blend(data, index, WHITE, clamp(stroke * 0.9 - beam, 0, 1) * 0.95);

      // Spectrum fan leaving it.
      for (let i = 0; i < spectrum.length; i++) {
        const spread = s * (0.12 + i * 0.075);
        const distance = distToSegment(
          x,
          y,
          s * 0.5,
          beamY,
          s * 0.92,
          beamY + spread
        );
        blend(data, index, spectrum[i], clamp(stroke * 0.42 - distance, 0, 1) * 0.55);
      }

      // Triangle outline with a subtle two-tone gradient.
      const edges = [
        distToSegment(x, y, apex[0], apex[1], left[0], left[1]),
        distToSegment(x, y, apex[0], apex[1], right[0], right[1]),
        distToSegment(x, y, left[0], left[1], right[0], right[1]),
      ];
      const distance = Math.min(...edges);
      const tone = ACCENT_2.map((c, i) => Math.round((c + ACCENT[i]) / 2 + (c - ACCENT[i]) * (x / s)));
      blend(data, index, tone, clamp(stroke - distance, 0, 1));
    }
  }
  return data;
}

function crc32(buffer) {
  let crc = 0xffffffff;
  for (let i = 0; i < buffer.length; i++) {
    crc ^= buffer[i];
    for (let bit = 0; bit < 8; bit++) {
      crc = crc & 1 ? (crc >>> 1) ^ 0xedb88320 : crc >>> 1;
    }
  }
  return (crc ^ 0xffffffff) >>> 0;
}

function chunk(type, payload) {
  const length = Buffer.alloc(4);
  length.writeUInt32BE(payload.length);
  const body = Buffer.concat([Buffer.from(type, "ascii"), payload]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(body));
  return Buffer.concat([length, body, crc]);
}

function encodePng(size, rgba) {
  const header = Buffer.alloc(13);
  header.writeUInt32BE(size, 0);
  header.writeUInt32BE(size, 4);
  header[8] = 8;
  header[9] = 6;
  const raw = Buffer.alloc(size * (size * 4 + 1));
  for (let y = 0; y < size; y++) {
    raw[y * (size * 4 + 1)] = 0;
    Buffer.from(rgba.buffer, y * size * 4, size * 4).copy(raw, y * (size * 4 + 1) + 1);
  }
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk("IHDR", header),
    chunk("IDAT", deflateSync(raw, { level: 9 })),
    chunk("IEND", Buffer.alloc(0)),
  ]);
}

// ICO entry using an uncompressed 32bpp BMP (works for every size Windows uses).
function encodeIcoEntry(size, rgba) {
  const pixels = Buffer.alloc(size * size * 4);
  for (let y = 0; y < size; y++) {
    for (let x = 0; x < size; x++) {
      const source = ((size - 1 - y) * size + x) * 4;
      const target = (y * size + x) * 4;
      pixels[target] = rgba[source + 2];
      pixels[target + 1] = rgba[source + 1];
      pixels[target + 2] = rgba[source];
      pixels[target + 3] = rgba[source + 3];
    }
  }
  const maskRow = Math.ceil(size / 32) * 4;
  const mask = Buffer.alloc(maskRow * size);
  const header = Buffer.alloc(40);
  header.writeUInt32LE(40, 0);
  header.writeInt32LE(size, 4);
  header.writeInt32LE(size * 2, 8);
  header.writeUInt16LE(1, 12);
  header.writeUInt16LE(32, 14);
  header.writeUInt32LE(0, 16);
  header.writeUInt32LE(pixels.length, 20);
  return Buffer.concat([header, pixels, mask]);
}

const png = (size) => encodePng(size, render(size));

for (const size of [32, 128, 256, 512]) {
  writeFileSync(join(outDir, size === 512 ? "icon.png" : `${size}x${size}.png`), png(size));
}
writeFileSync(join(outDir, "128x128@2x.png"), png(256));
writeFileSync(join(outDir, "icon.png"), png(512));

const icoSizes = [16, 24, 32, 48, 64, 128, 256];
const entries = icoSizes.map((size) => ({ size, data: encodeIcoEntry(size, render(size)) }));
const header = Buffer.alloc(6);
header.writeUInt16LE(0, 0);
header.writeUInt16LE(1, 2);
header.writeUInt16LE(entries.length, 4);
const directory = Buffer.alloc(16 * entries.length);
let offset = 6 + directory.length;
entries.forEach((entry, index) => {
  const base = index * 16;
  directory[base] = entry.size === 256 ? 0 : entry.size;
  directory[base + 1] = entry.size === 256 ? 0 : entry.size;
  directory[base + 2] = 0;
  directory[base + 3] = 0;
  directory.writeUInt16LE(1, base + 4);
  directory.writeUInt16LE(32, base + 6);
  directory.writeUInt32LE(entry.data.length, base + 8);
  directory.writeUInt32LE(offset, base + 12);
  offset += entry.data.length;
});
writeFileSync(
  join(outDir, "icon.ico"),
  Buffer.concat([header, directory, ...entries.map((entry) => entry.data)])
);

console.log(`icons written to ${outDir}`);
