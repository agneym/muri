export interface UnusedFilesOptions {
  /**
   * Entry point files or glob patterns
   * @example ['src/index.ts'] or 'src/index.ts'
   */
  entry: string | string[];

  /**
   * Project files to check (glob patterns)
   * @default ['**\/*.{ts,tsx,js,jsx,mjs,cjs}']
   */
  project?: string | string[];

  /**
   * Working directory
   * @default process.cwd()
   */
  cwd?: string;

  /**
   * Patterns to ignore
   */
  ignore?: string[];

  /**
   * Include files from node_modules
   * @default false
   */
  includeNodeModules?: boolean;
}

export interface UnusedFilesReport {
  /**
   * List of unused file paths (relative to cwd)
   */
  unusedFiles: string[];

  /**
   * Total number of project files analyzed
   */
  totalFiles: number;

  /**
   * Number of unused files found
   */
  unusedCount: number;
}

/**
 * Find unused files in a JavaScript/TypeScript project
 *
 * @example
 * ```ts
 * import { findUnused } from 'muri';
 *
 * const report = await findUnused({
 *   entry: ['src/index.ts'],
 *   cwd: '/path/to/project',
 * });
 *
 * console.log(`Found ${report.unusedCount} unused files`);
 * for (const file of report.unusedFiles) {
 *   console.log(`  ${file}`);
 * }
 * ```
 */
export function findUnused(options: UnusedFilesOptions): Promise<UnusedFilesReport>;

/**
 * Find unused files in a JavaScript/TypeScript project (synchronous)
 *
 * @example
 * ```ts
 * import { findUnusedSync } from 'muri';
 *
 * const report = findUnusedSync({
 *   entry: ['src/index.ts'],
 * });
 * ```
 */
export function findUnusedSync(options: UnusedFilesOptions): UnusedFilesReport;

/**
 * Find all files reachable from entry points
 *
 * Returns the set of files that are directly or transitively imported
 * from the specified entry points.
 *
 * @example
 * ```ts
 * import { findReachable } from 'muri';
 *
 * const files = await findReachable({
 *   entry: ['src/index.ts'],
 * });
 *
 * console.log(`${files.length} files are reachable from entry points`);
 * ```
 */
export function findReachable(options: UnusedFilesOptions): Promise<string[]>;
