# unused-files

A fast CLI tool to detect unused files in JavaScript/TypeScript projects.

Built with [oxc](https://oxc.rs/) for fast parsing and parallel processing with [rayon](https://github.com/rayon-rs/rayon).

## Installation

```bash
cargo install --path .
```

## Usage

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

## How It Works

1. Collects all project files matching the project glob patterns
2. Parses entry points and recursively resolves all imports
3. Builds a dependency graph of all reachable files
4. Reports files that exist in the project but aren't reachable from any entry point

## License

MIT
