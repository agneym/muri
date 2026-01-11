# muri

A fast tool to detect unused files in JavaScript/TypeScript projects.

Built with [oxc](https://oxc.rs/) for fast parsing and parallel processing with [rayon](https://github.com/rayon-rs/rayon).

## Installation

### npm

```bash
npm install muri
```

### Cargo

```bash
cargo install --path .
```

## CLI Usage

```bash
muri --entry src/index.ts
```

### Options

| Option | Description | Default |
|--------|-------------|---------|
| `-e, --entry <PATTERN>` | Entry point files or glob patterns (required) | - |
| `-p, --project <PATTERN>` | Project files to check | `**/*.{ts,tsx,js,jsx,mjs,cjs}` |
| `-C, --cwd <PATH>` | Working directory | `.` |
| `-c, --config <PATH>` | Path to config file | - |
| `--format <FORMAT>` | Output format: `text` or `json` | `text` |
| `--ignore <PATTERN>` | Patterns to ignore | - |

### Configuration File

Muri supports configuration via `muri.json` or `muri.jsonc` files. If no `--config` flag is provided, muri automatically looks for these files in the working directory.

**Supported formats:**
- JSON (`.json`)
- JSON with Comments (`.jsonc`)

**Example `muri.json`:**

```json
{
  "entry": ["src/index.ts", "src/worker.ts"],
  "project": ["src/**/*.ts", "src/**/*.tsx"],
  "ignore": ["**/*.test.ts", "**/*.spec.ts"]
}
```

**Example `muri.jsonc` (with comments):**

```jsonc
{
  // Entry points for dependency analysis
  "entry": ["src/index.ts"],

  // Files to check for unused status
  "project": ["src/**/*.ts", "src/**/*.tsx"],

  // Patterns to exclude
  "ignore": ["**/*.test.ts", "**/*.spec.ts"]
}
```

| Option | Type | Description |
|--------|------|-------------|
| `entry` | `string[]` | Entry point files or glob patterns |
| `project` | `string[]` | Project files to check |
| `ignore` | `string[]` | Patterns to ignore |

CLI arguments override config file values when both are provided.

### Examples

Find unused files in a React project:

```bash
muri --entry "src/index.tsx"
```

Multiple entry points:

```bash
muri --entry "src/main.ts" --entry "src/worker.ts"
```

With custom project scope and ignoring test files:

```bash
muri \
  --entry "src/index.ts" \
  --project "src/**/*.ts" \
  --ignore "**/*.test.ts" \
  --ignore "**/*.spec.ts"
```

JSON output for CI/tooling:

```bash
muri --entry "src/index.ts" --format json
```

## Exit Codes

- `0` - No unused files found
- `1` - Unused files detected or error occurred

## Node.js API

### findUnused

Find unused files in a project (async).

```js
const { findUnused } = require('muri');

const report = await findUnused({
  entry: 'src/index.ts',
  ignore: ['**/*.test.ts'],
});

console.log(`Found ${report.unusedCount} unused files out of ${report.totalFiles}`);
report.unusedFiles.forEach(file => console.log(`  ${file}`));
```

### findUnusedSync

Synchronous version of `findUnused`.

```js
const { findUnusedSync } = require('muri');

const report = findUnusedSync({
  entry: ['src/index.ts', 'src/worker.ts'],
  project: 'src/**/*.ts',
  cwd: '/path/to/project',
});
```

### findReachable

Find all files reachable from entry points (async).

```js
const { findReachable } = require('muri');

const files = await findReachable({
  entry: 'src/index.ts',
});

console.log(`${files.length} files are reachable from entry points`);
```

### Options

| Option | Type | Description | Default |
|--------|------|-------------|---------|
| `entry` | `string \| string[]` | Entry point files or glob patterns | (required) |
| `project` | `string \| string[]` | Project files to check | `**/*.{ts,tsx,js,jsx,mjs,cjs}` |
| `cwd` | `string` | Working directory | `process.cwd()` |
| `ignore` | `string[]` | Patterns to ignore | `[]` |

## How It Works

1. Collects all project files matching the project glob patterns
2. Parses entry points and recursively resolves all imports
3. Builds a dependency graph of all reachable files
4. Reports files that exist in the project but aren't reachable from any entry point

## License

MIT
