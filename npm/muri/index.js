const { platform, arch } = process;
const path = require('path');

const PLATFORM_PACKAGES = {
  'darwin-x64': 'muri-darwin-x64',
  'darwin-arm64': 'muri-darwin-arm64',
  'linux-x64': 'muri-linux-x64-gnu',
  'linux-arm64': 'muri-linux-arm64-gnu',
  'win32-x64': 'muri-win32-x64-msvc',
};

function getPlatformPackage() {
  const key = `${platform}-${arch}`;

  // First, try to load local development build
  try {
    const localPath = path.join(__dirname, 'native.js');
    return require(localPath);
  } catch (e) {
    // Local build not found, try platform package
  }

  const packageName = PLATFORM_PACKAGES[key];

  if (!packageName) {
    throw new Error(
      `Unsupported platform: ${key}. ` +
      `Supported platforms: ${Object.keys(PLATFORM_PACKAGES).join(', ')}`
    );
  }

  try {
    return require(packageName);
  } catch (e) {
    throw new Error(
      `Failed to load native module for platform ${key}. ` +
      `Package: ${packageName}. ` +
      `Please ensure the package is installed: npm install ${packageName}\n` +
      `Original error: ${e.message}`
    );
  }
}

let nativeModule;

function getNativeModule() {
  if (!nativeModule) {
    nativeModule = getPlatformPackage();
  }
  return nativeModule;
}

/**
 * Find unused files in a JavaScript/TypeScript project
 * @param {Object} options - Configuration options
 * @param {string|string[]} options.entry - Entry point files or glob patterns
 * @param {string|string[]} [options.project] - Project files to check (glob patterns)
 * @param {string} [options.cwd] - Working directory (defaults to current directory)
 * @param {string[]} [options.ignore] - Patterns to ignore
 * @param {boolean} [options.includeNodeModules] - Include files from node_modules
 * @returns {Promise<{unusedFiles: string[], totalFiles: number, unusedCount: number}>}
 */
async function findUnused(options) {
  const native = getNativeModule();

  // Normalize entry to array
  const entry = Array.isArray(options.entry) ? options.entry : [options.entry];

  // Normalize project to array
  const project = options.project
    ? (Array.isArray(options.project) ? options.project : [options.project])
    : undefined;

  return native.findUnused({
    entry,
    project,
    cwd: options.cwd,
    ignore: options.ignore,
    includeNodeModules: options.includeNodeModules,
  });
}

/**
 * Find unused files in a JavaScript/TypeScript project (sync)
 * @param {Object} options - Configuration options
 * @param {string|string[]} options.entry - Entry point files or glob patterns
 * @param {string|string[]} [options.project] - Project files to check (glob patterns)
 * @param {string} [options.cwd] - Working directory (defaults to current directory)
 * @param {string[]} [options.ignore] - Patterns to ignore
 * @param {boolean} [options.includeNodeModules] - Include files from node_modules
 * @returns {{unusedFiles: string[], totalFiles: number, unusedCount: number}}
 */
function findUnusedSync(options) {
  const native = getNativeModule();

  const entry = Array.isArray(options.entry) ? options.entry : [options.entry];
  const project = options.project
    ? (Array.isArray(options.project) ? options.project : [options.project])
    : undefined;

  return native.findUnusedSync({
    entry,
    project,
    cwd: options.cwd,
    ignore: options.ignore,
    includeNodeModules: options.includeNodeModules,
  });
}

/**
 * Find all files reachable from entry points
 * @param {Object} options - Configuration options
 * @param {string|string[]} options.entry - Entry point files or glob patterns
 * @param {string|string[]} [options.project] - Project files to check (glob patterns)
 * @param {string} [options.cwd] - Working directory (defaults to current directory)
 * @param {string[]} [options.ignore] - Patterns to ignore
 * @param {boolean} [options.includeNodeModules] - Include files from node_modules
 * @returns {Promise<string[]>}
 */
async function findReachable(options) {
  const native = getNativeModule();

  const entry = Array.isArray(options.entry) ? options.entry : [options.entry];
  const project = options.project
    ? (Array.isArray(options.project) ? options.project : [options.project])
    : undefined;

  return native.findReachable({
    entry,
    project,
    cwd: options.cwd,
    ignore: options.ignore,
    includeNodeModules: options.includeNodeModules,
  });
}

module.exports = {
  findUnused,
  findUnusedSync,
  findReachable,
};
