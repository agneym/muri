// Entry point with various import patterns
import { staticHelper } from './static-import';
import type { SomeType } from './types';

// Dynamic import
async function loadLazy() {
  const mod = await import('./lazy-module');
  return mod.default;
}

// Re-export usage
export * from './barrel';

console.log(staticHelper());
loadLazy();
