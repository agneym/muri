## Overview

Muri is a fast tool to detect unused files in JavaScript/TypeScript projects. Built with Rust for performance using oxc for parsing and rayon for parallel processing. Provides both a CLI and Node.js API via NAPI bindings.

## Build & Development

Use pnpm as package manager for Node.js

```bash
pnpm run build              # Build NAPI + CLI binary
pnpm run build:debug        # Debug build of NAPI only
pnpm run test               # Run Node.js API tests (tests/node-api.test.js)
cargo check -p muri        # Quick type check core library
cargo test -p muri         # Run Rust unit tests
cargo build --release -p muri-cli  # Build CLI binary only
```

Pre-commit hooks (lefthook) run `cargo fmt --check` and `cargo clippy` automatically.

## Architecture

Three Rust crates in a workspace:

**`crates/muri/`** - Core library with analysis pipeline:
- `collector.rs` - Single-pass filesystem walk with precompiled glob matchers
- `parser.rs` - Import extraction from JS/TS files
- `resolver.rs` - Module path resolution (path mapping, index files)
- `graph.rs` - Dependency graph construction with parallel wave-based traversal
- `module_cache.rs` - Caches parsed modules to avoid re-parsing
- `plugin/` - Extensible system for entry point discovery from tool configs (Storybook plugin included)

**`crates/muri-cli/`** - CLI binary, parses args and optional `muri.json`/`muri.jsonc` config

**`crates/muri-napi/`** - Node.js NAPI bindings exposing `findUnused()`, `findUnusedSync()`, `findReachable()`

**`npm/muri/`** - npm package wrapper with platform-specific binary resolution

## Key Code Patterns

- Use `FxHashMap`/`FxHashSet` from `rustc_hash` instead of std collections (enforced by clippy)
- Plugins auto-detect from package.json dependencies; can be overridden in config
- Entry points and project files use glob patterns compiled once upfront
- Graph traversal processes files in parallel waves using rayon
- Foreign file imports (CSS, images, etc.) are resolved but not parsed - a warning is printed

## Configuration

Config file (`muri.json` or `muri.jsonc`) example:
```jsonc
{
  "entry": ["src/index.ts"],
  "project": ["src/**/*.ts"],
  "ignore": ["**/*.test.ts"],
  "plugins": { "storybook": true }
}
```

CLI args override config file values.
