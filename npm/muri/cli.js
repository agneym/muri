#!/usr/bin/env node

const { findUnusedSync } = require('./index');

function parseArgs(args) {
  const options = {
    entry: [],
    project: [],
    cwd: process.cwd(),
    ignore: [],
    format: 'text',
  };

  function requireValue(flag) {
    console.error(`Error: ${flag} requires a value`);
    process.exit(1);
  }

  let i = 0;
  while (i < args.length) {
    const arg = args[i];

    if (arg === '-e' || arg === '--entry') {
      i++;
      if (i >= args.length || args[i].startsWith('-')) {
        requireValue(arg);
      }
      options.entry.push(args[i]);
    } else if (arg === '-p' || arg === '--project') {
      i++;
      if (i >= args.length || args[i].startsWith('-')) {
        requireValue(arg);
      }
      options.project.push(args[i]);
    } else if (arg === '-C' || arg === '--cwd') {
      i++;
      if (i >= args.length || args[i].startsWith('-')) {
        requireValue(arg);
      }
      options.cwd = args[i];
    } else if (arg === '--ignore') {
      i++;
      if (i >= args.length || args[i].startsWith('-')) {
        requireValue(arg);
      }
      options.ignore.push(args[i]);
    } else if (arg === '--format') {
      i++;
      if (i >= args.length || args[i].startsWith('-')) {
        requireValue(arg);
      }
      options.format = args[i];
    } else if (arg === '-h' || arg === '--help') {
      printHelp();
      process.exit(0);
    } else if (arg === '-V' || arg === '--version') {
      const pkg = require('./package.json');
      console.log(pkg.version);
      process.exit(0);
    }

    i++;
  }

  return options;
}

function printHelp() {
  console.log(`muri - Find unused files in JS/TS projects

USAGE:
    muri [OPTIONS]

OPTIONS:
    -e, --entry <PATTERN>      Entry point files or glob patterns (required, can be repeated)
    -p, --project <PATTERN>    Project files to check (default: **/*.{ts,tsx,js,jsx,mjs,cjs})
    -C, --cwd <PATH>           Working directory (default: .)
    --ignore <PATTERN>         Patterns to ignore (can be repeated)
    --format <FORMAT>          Output format: text or json (default: text)
    -h, --help                 Print help
    -V, --version              Print version

EXAMPLES:
    muri --entry src/index.ts
    muri --entry src/main.ts --project "src/**/*.ts" --ignore "**/*.test.ts"
    muri --entry src/index.ts --format json`);
}

function main() {
  const args = process.argv.slice(2);
  const options = parseArgs(args);

  if (options.entry.length === 0) {
    console.error('Error: At least one --entry is required');
    console.error('Run with --help for usage information');
    process.exit(1);
  }

  try {
    const result = findUnusedSync({
      entry: options.entry,
      project: options.project.length > 0 ? options.project : undefined,
      cwd: options.cwd,
      ignore: options.ignore,
    });

    if (options.format === 'json') {
      console.log(JSON.stringify(result, null, 2));
    } else {
      if (result.unusedCount === 0) {
        console.log('No unused files found.');
      } else {
        console.log(`Unused files (${result.unusedCount}):`);
        for (const file of result.unusedFiles) {
          console.log(`  ${file}`);
        }
        console.log(`\n${result.unusedCount}/${result.totalFiles} files unused`);
      }
    }

    // Exit with error code if unused files found
    if (result.unusedCount > 0) {
      process.exit(1);
    }
  } catch (error) {
    console.error(`Error: ${error.message}`);
    process.exit(1);
  }
}

main();
