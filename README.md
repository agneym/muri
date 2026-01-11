# unused-files

A fast tool to detect unused files in JavaScript/TypeScript projects.

Built with [oxc](https://oxc.rs/) for fast parsing and parallel processing with [rayon](https://github.com/rayon-rs/rayon).

## Installation

### npm

```bash
npm install unused-files
```

### Cargo

```bash
cargo install --path .
```

## CLI Usage

```bash
unused-files --entry src/index.ts
```

### Options

| Option | Description | Default |
|--------|-------------|---------|
| `-e, --entry <PATTERN>` | Entry point files or glob patterns (required) | - |
| `-p, --project <PATTERN>` | Project files to check | `**/*.{ts,tsx,js,jsx,mjs,cjs}` |
| `-C, --cwd <PATH>` | Working directory | `.` |
| `--format <FORMAT>` | Output format: `text` or `json` | `text` |
| `--ignore <PATTERN>` | Patterns to ignore | - |
| `--include-node-modules` | Include files from node_modules | `false` |

### Examples

Find unused files in a React project:

```bash
unused-files --entry "src/index.tsx"
```

Multiple entry points:

```bash
unused-files --entry "src/main.ts" --entry "src/worker.ts"
```

With custom project scope and ignoring test files:

```bash
unused-files \
  --entry "src/index.ts" \
  --project "src/**/*.ts" \
  --ignore "**/*.test.ts" \
  --ignore "**/*.spec.ts"
```

JSON output for CI/tooling:

```bash
unused-files --entry "src/index.ts" --format json
```

## Exit Codes

- `0` - No unused files found
- `1` - Unused files detected or error occurred

## Node.js API

### findUnused

Find unused files in a project (async).

```js
const { findUnused } = require('unused-files');

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
const { findUnusedSync } = require('unused-files');

const report = findUnusedSync({
  entry: ['src/index.ts', 'src/worker.ts'],
  project: 'src/**/*.ts',
  cwd: '/path/to/project',
});
```

### findReachable

Find all files reachable from entry points (async).

```js
const { findReachable } = require('unused-files');

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
| `includeNodeModules` | `boolean` | Include files from node_modules | `false` |

## How It Works

1. Collects all project files matching the project glob patterns
2. Parses entry points and recursively resolves all imports
3. Builds a dependency graph of all reachable files
4. Reports files that exist in the project but aren't reachable from any entry point

## License

MIT
