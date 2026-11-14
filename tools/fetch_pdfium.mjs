#!/usr/bin/env node
// Prepare the PDFium runtime library used by the PDF reader.
//
// The Tauri bundle configuration references `src-tauri/libs/`, but the binaries
// are platform-specific and intentionally not committed. This script downloads
// the pinned build from bblanchon/pdfium-binaries (the same source as the local
// development copy) and places the shared library where `pdfium-render` looks
// for it at runtime.
//
// Source:  https://github.com/bblanchon/pdfium-binaries
// Version: chromium/7891 (PDFium 151.0.7891.0)
// License: BSD-3-Clause (PDFium Authors)
//
// Usage:
//   node tools/fetch_pdfium.mjs          # download if missing
//   node tools/fetch_pdfium.mjs --force  # re-download

import { spawn } from 'node:child_process';
import { copyFile, mkdir, readdir, rm, stat } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const VERSION = process.env.PDFIUM_VERSION || '7891';
const BASE = `https://github.com/bblanchon/pdfium-binaries/releases/download/chromium%2F${VERSION}`;
const HERE = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(HERE, '..');
const LIBS_DIR = join(ROOT, 'src-tauri', 'libs');
const TEMP_DIR = join(LIBS_DIR, '.pdfium-download');
const force = process.argv.includes('--force');

const TARGETS = {
  'win32-x64': { archive: 'pdfium-win-x64.tgz', file: 'pdfium.dll' },
  'linux-x64': { archive: 'pdfium-linux-x64.tgz', file: 'libpdfium.so' },
};

const key = `${process.platform}-${process.arch}`;
const target = TARGETS[key];
if (!target) {
  console.error(`[pdfium] unsupported platform ${key}; supported: ${Object.keys(TARGETS).join(', ')}`);
  process.exit(1);
}

const destination = join(LIBS_DIR, target.file);

async function sizeOf(path) {
  try {
    return (await stat(path)).size;
  } catch {
    return 0;
  }
}

function run(command, args, options = {}) {
  return new Promise((resolvePromise) => {
    const child = spawn(command, args, { stdio: 'inherit', ...options });
    child.on('close', (code) => resolvePromise(code ?? 1));
    child.on('error', () => resolvePromise(1));
  });
}

async function findFile(root, name) {
  for (const entry of await readdir(root, { withFileTypes: true })) {
    const path = join(root, entry.name);
    if (entry.isDirectory()) {
      const found = await findFile(path, name);
      if (found) return found;
    } else if (entry.name === name) {
      return path;
    }
  }
  return null;
}

async function main() {
  await mkdir(LIBS_DIR, { recursive: true });

  const existing = await sizeOf(destination);
  if (!force && existing > 5 * 1024 * 1024) {
    console.log(`[pdfium] ${target.file} already present (${(existing / 1048576).toFixed(1)} MB), skipping`);
    return;
  }

  const url = `${BASE}/${target.archive}`;
  const archive = join(TEMP_DIR, target.archive);
  await rm(TEMP_DIR, { recursive: true, force: true });
  await mkdir(TEMP_DIR, { recursive: true });

  console.log(`[pdfium] downloading ${url}`);
  const downloadCode = await run('curl', [
    '-fL',
    '--retry', '3',
    '--connect-timeout', '30',
    '--speed-time', '60',
    '--speed-limit', '1024',
    '-o', archive,
    url,
  ]);
  if (downloadCode !== 0) {
    throw new Error(`curl failed with exit code ${downloadCode}`);
  }

  const archiveSize = await sizeOf(archive);
  if (archiveSize < 2 * 1024 * 1024) {
    throw new Error(`downloaded archive looks invalid (${archiveSize} bytes)`);
  }

  console.log(`[pdfium] extracting ${target.archive}`);
  const extractCode = await run('tar', ['-xzf', archive, '-C', TEMP_DIR]);
  if (extractCode !== 0) {
    throw new Error(`tar failed with exit code ${extractCode}`);
  }

  const extracted = await findFile(TEMP_DIR, target.file);
  if (!extracted) {
    throw new Error(`${target.file} not found in ${target.archive}`);
  }

  const extractedSize = await sizeOf(extracted);
  if (extractedSize < 5 * 1024 * 1024) {
    throw new Error(`extracted ${target.file} looks invalid (${extractedSize} bytes)`);
  }

  await copyFile(extracted, destination);
  await rm(TEMP_DIR, { recursive: true, force: true });
  console.log(`[pdfium] ready: ${destination} (${(extractedSize / 1048576).toFixed(1)} MB)`);
}

main().catch((error) => {
  console.error(`[pdfium] ${error.message}`);
  process.exit(1);
});
