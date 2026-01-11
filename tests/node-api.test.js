/**
 * Test script for unused-files Node.js API
 *
 * Run after building the native module:
 *   npm run build
 *   node tests/node-api.test.js
 */

const path = require('path');
const { findUnused, findUnusedSync, findReachable } = require('../npm/muri');

const testProjectPath = path.join(__dirname, 'fixtures', 'node-api');

async function runTests() {
  console.log('Testing unused-files Node.js API\n');
  console.log('Test project:', testProjectPath);
  console.log('---');

  // Test async findUnused
  console.log('\n1. Testing findUnused (async):');
  try {
    const result = await findUnused({
      entry: ['src/index.ts'],
      cwd: testProjectPath,
    });
    console.log('  Result:', JSON.stringify(result, null, 2));
    console.log('  ✓ findUnused works');
  } catch (e) {
    console.error('  ✗ findUnused failed:', e.message);
  }

  // Test sync findUnusedSync
  console.log('\n2. Testing findUnusedSync (sync):');
  try {
    const result = findUnusedSync({
      entry: ['src/index.ts'],
      cwd: testProjectPath,
    });
    console.log('  Result:', JSON.stringify(result, null, 2));
    console.log('  ✓ findUnusedSync works');
  } catch (e) {
    console.error('  ✗ findUnusedSync failed:', e.message);
  }

  // Test findReachable
  console.log('\n3. Testing findReachable (async):');
  try {
    const files = await findReachable({
      entry: ['src/index.ts'],
      cwd: testProjectPath,
    });
    console.log('  Reachable files:', files);
    console.log('  ✓ findReachable works');
  } catch (e) {
    console.error('  ✗ findReachable failed:', e.message);
  }

  // Test with string entry (not array)
  console.log('\n4. Testing with single string entry:');
  try {
    const result = await findUnused({
      entry: 'src/index.ts',
      cwd: testProjectPath,
    });
    console.log('  Result:', JSON.stringify(result, null, 2));
    console.log('  ✓ String entry works');
  } catch (e) {
    console.error('  ✗ String entry failed:', e.message);
  }

  // Test with ignore patterns
  console.log('\n5. Testing with ignore patterns:');
  try {
    const result = await findUnused({
      entry: ['src/index.ts'],
      cwd: testProjectPath,
      ignore: ['**/unused.ts'],
    });
    console.log('  Result:', JSON.stringify(result, null, 2));
    console.log('  ✓ Ignore patterns work');
  } catch (e) {
    console.error('  ✗ Ignore patterns failed:', e.message);
  }

  console.log('\n---');
  console.log('Tests complete!');
}

runTests().catch(console.error);
