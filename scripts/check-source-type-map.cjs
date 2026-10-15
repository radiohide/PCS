'use strict';
// The plugins page shows which objects each source provides, from a generated file. A generated
// file that has gone stale is worse than a hand-written one: it is trusted. This runs the generator
// in its check mode, so the page cannot describe a mapping the repository no longer has.
const { execFileSync } = require('node:child_process');
const path = require('node:path');
const root = path.resolve(__dirname, '..');
try {
  const output = execFileSync(process.execPath, [path.join(__dirname, 'build-source-type-map.cjs'), '--check'], { cwd: root, encoding: 'utf8' });
  process.stdout.write(output);
} catch (error) {
  process.stderr.write(error.stdout || '');
  process.stderr.write(error.stderr || '');
  process.exit(1);
}
