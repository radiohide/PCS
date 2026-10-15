'use strict';
// S23: the object model's renderer may only read paths the query contract defines.
//
// `model-view.js` declares every contract path it reads in `PATHS`. This checks each one against
// the fixed response set R05 produced, so a renamed field fails here and names the path instead of
// rendering a blank cell on a page nobody happened to open. It is the same arrangement
// `check-drill.cjs` uses for the drill's stand-in renderer: the dependency is written down and
// verified, rather than left to whoever reads both files to notice.
const fs = require('node:fs');
const path = require('node:path');
const root = path.resolve(__dirname, '..');
const p = (...parts) => path.join(root, ...parts);

const view = require(p('apps/desktop/ui/model-view.js'));
const samples = JSON.parse(fs.readFileSync(p('model/query-samples.json'), 'utf8')).samples;

// Walk a `a[].b.c` expression over one response. The same shape `check-drill.cjs` resolves: `[]`
// spreads an array, and a missing segment means the path is not there.
function resolvePath(source, expression) {
  let current = [source];
  for (const segment of expression.split('.')) {
    const spread = segment.endsWith('[]');
    const field = spread ? segment.slice(0, -2) : segment;
    const next = [];
    for (const value of current) {
      if (value === null || value === undefined) continue;
      const child = value[field];
      if (child === undefined) continue;
      if (spread) {
        if (!Array.isArray(child)) continue;
        next.push(...child);
      } else {
        next.push(child);
      }
    }
    current = next;
    if (current.length === 0) return false;
  }
  return current.length > 0;
}

const failures = [];
// The mock marks its responses; the paths are the contract's either way, and the fixed set is what
// the contract says a real response looks like.
const responses = samples.map((sample) => sample.response);
for (const expression of view.PATHS) {
  const found = responses.some((response) => resolvePath(response, expression));
  if (!found) failures.push(expression);
}

// Optional paths are checked too, and they are checked to be *declared*: the contract omits a
// field when it is empty, so absence from the fixed set is not evidence that the field was renamed.
// Listing them with a reason is what keeps them from becoming a place a renamed field could hide.
for (const entry of view.OPTIONAL_PATHS || []) {
  const [expression, reason] = entry;
  if (!expression || !reason || reason.length < 10) failures.push(`OPTIONAL_PATHS 条目缺少原因：${expression}`);
  if (view.PATHS.includes(expression)) failures.push(`${expression} 同时出现在 PATHS 与 OPTIONAL_PATHS 中——二者只能取一`);
}
// A renderer that declares no paths would pass every check above while reading whatever it liked.
if (view.PATHS.length < 20) failures.push(`PATHS 只声明了 ${view.PATHS.length} 条路径，不足以描述这个渲染器读取的字段`);

console.log(`Renderer contract paths: ${view.PATHS.length} checked against ${responses.length} fixed responses, ${(view.OPTIONAL_PATHS||[]).length} declared optional`);
if (failures.length) {
  console.error(`\nModel view FAILED (${failures.length}):`);
  for (const failure of failures) console.error(`  ! 渲染器读取的契约路径 ${failure} 在固定响应集中不存在——字段被改名或未产出`);
  process.exit(1);
}
console.log('Model view verified.');
