#!/usr/bin/env node
/**
 * CLI spawner that delegates to the native muri binary.
 * Tries local binary first (for development), then platform-specific package.
 */

const { execFileSync } = require('child_process');
const fs = require('fs');
const path = require('path');

const PLATFORM_PACKAGES = {
  'darwin-x64': 'muri-darwin-x64',
  'darwin-arm64': 'muri-darwin-arm64',
  'linux-x64': 'muri-linux-x64-gnu',
  'linux-arm64': 'muri-linux-arm64-gnu',
  'win32-x64': 'muri-win32-x64-msvc',
};

function findBinary() {
  // Try local binary first (development)
  const localBinary = path.join(__dirname, 'muri');
  if (fs.existsSync(localBinary)) {
    return localBinary;
  }

  // Try platform-specific package
  const platformKey = `${process.platform}-${process.arch}`;
  const pkgName = PLATFORM_PACKAGES[platformKey];

  if (!pkgName) {
    console.error(`Unsupported platform: ${platformKey}`);
    process.exit(1);
  }

  try {
    const pkgPath = require.resolve(`${pkgName}/package.json`);
    const isWindows = process.platform === 'win32';
    const binaryName = isWindows ? 'muri.exe' : 'muri';
    return path.join(path.dirname(pkgPath), binaryName);
  } catch {
    console.error(`Could not find muri binary for platform: ${platformKey}`);
    console.error(`Make sure ${pkgName} is installed.`);
    process.exit(1);
  }
}

const binPath = findBinary();

try {
  execFileSync(binPath, process.argv.slice(2), { stdio: 'inherit' });
} catch (e) {
  process.exit(e.status ?? 1);
}
