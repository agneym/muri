#!/usr/bin/env node
/**
 * Post-prepublish script to add CLI binary to platform packages.
 * Run after `napi prepublish` to update each platform package with:
 * 1. The muri CLI binary
 * 2. Updated package.json with bin and files entries
 */

const fs = require('fs');
const path = require('path');

const npmDir = path.join(__dirname, '..', 'npm');

// Map of platform package names to their target triples
const PLATFORM_TARGETS = {
  'muri-darwin-x64': 'x86_64-apple-darwin',
  'muri-darwin-arm64': 'aarch64-apple-darwin',
  'muri-linux-x64-gnu': 'x86_64-unknown-linux-gnu',
  'muri-linux-arm64-gnu': 'aarch64-unknown-linux-gnu',
  'muri-linux-x64-musl': 'x86_64-unknown-linux-musl',
  'muri-linux-arm64-musl': 'aarch64-unknown-linux-musl',
  'muri-win32-x64-msvc': 'x86_64-pc-windows-msvc',
};

function updatePlatformPackage(pkgDir, pkgName) {
  const pkgJsonPath = path.join(pkgDir, 'package.json');

  if (!fs.existsSync(pkgJsonPath)) {
    console.log(`Skipping ${pkgName}: package.json not found`);
    return;
  }

  const pkgJson = JSON.parse(fs.readFileSync(pkgJsonPath, 'utf8'));

  // Add bin field pointing to the muri binary
  const isWindows = pkgName.includes('win32');
  const binaryName = isWindows ? 'muri.exe' : 'muri';

  pkgJson.bin = { muri: binaryName };

  // Update files array to include the binary
  if (!pkgJson.files) {
    pkgJson.files = [];
  }
  if (!pkgJson.files.includes(binaryName)) {
    pkgJson.files.push(binaryName);
  }

  fs.writeFileSync(pkgJsonPath, JSON.stringify(pkgJson, null, 2) + '\n');
  console.log(`Updated ${pkgName}/package.json`);

  // Copy binary if it exists in the target directory
  const target = PLATFORM_TARGETS[pkgName];
  if (target) {
    const binarySource = path.join(__dirname, '..', 'target', target, 'release', binaryName);
    const binaryDest = path.join(pkgDir, binaryName);

    if (fs.existsSync(binarySource)) {
      fs.copyFileSync(binarySource, binaryDest);
      // Make executable on Unix
      if (!isWindows) {
        fs.chmodSync(binaryDest, 0o755);
      }
      console.log(`Copied binary to ${pkgName}/${binaryName}`);
    } else {
      console.log(`Binary not found for ${pkgName}: ${binarySource}`);
    }
  }
}

// Find and process all platform packages
const entries = fs.readdirSync(npmDir, { withFileTypes: true });

for (const entry of entries) {
  if (entry.isDirectory() && entry.name.startsWith('muri-')) {
    const pkgDir = path.join(npmDir, entry.name);
    updatePlatformPackage(pkgDir, entry.name);
  }
}

console.log('Post-prepublish complete');
