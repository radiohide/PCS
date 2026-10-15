'use strict';
// S21: every source and every type has a decided handling path.
//
// The reconciliation is a comparison of three machine-readable things, not of a document someone
// kept in step by hand:
//
//   * what the adapters declare        — model/adapter-coverage.json, written by the Rust example
//   * which sources exist and where they go — model/source-map.cjs
//   * which types the catalog declares — model/catalog.json
//
// Anything the comparison cannot cover must be named in model/adapter-exceptions.cjs with a reason.
// That is the point: a source cannot go missing by being quietly skipped, and coverage cannot be
// reached by deleting the entries that were hard to map.
const fs = require('node:fs');
const path = require('node:path');
const root = path.resolve(__dirname, '..');
const p = (...parts) => path.join(root, ...parts);

const coverage = JSON.parse(fs.readFileSync(p('model/adapter-coverage.json'), 'utf8'));
const catalog = JSON.parse(fs.readFileSync(p('model/catalog.json'), 'utf8'));
const { rows } = require(p('model/source-map.cjs'));
const { sourceExceptions, typeExceptions } = require(p('model/adapter-exceptions.cjs'));

const failures = [];
const check = (condition, message) => { if (!condition) failures.push(message); };

const declared = new Set(coverage.declared.map((entry) => entry.source));
const declaredTypes = new Set(coverage.declared.map((entry) => entry.type_id).filter(Boolean));

// The event path is a rule, not a list: the adapter decides by prefix, and the same rule is what
// this check applies, so the two cannot drift into disagreeing about which sources are events.
const isEventSource = (source) =>
  !coverage.event_exclusions.includes(source) &&
  ((coverage.event_sources || []).includes(source) || source.startsWith(coverage.event_prefix) || source.startsWith(coverage.usn_prefix));

// ---------- every source has a decided path ----------
const sources = new Map();
for (const row of rows) {
  const [source, destination, types] = row.split('|');
  if (!sources.has(source)) sources.set(source, { destination, types: (types || '').split(',').map((t) => t.trim()).filter(t=>t && t!=='-') });
}
const uncovered = [];
for (const [source, entry] of sources) {
  if (declared.has(source)) continue;
  if (isEventSource(source)) continue;
  if (sourceExceptions[source]) continue;
  uncovered.push(`${source}（去向 ${entry.destination}）`);
}
check(uncovered.length === 0, `以下来源既没有适配器声明，也没有在 model/adapter-exceptions.cjs 里给出原因：${uncovered.join('、')}`);

// A declared adapter for a source that no longer exists is the other half of the same problem.
const phantom = [...declared].filter((source) => !sources.has(source));
check(phantom.length === 0, `以下来源声明了适配器，但来源映射里没有它：${phantom.join('、')}`);

// ---------- the declared path matches the destination the inventory states ----------
const mismatch = [];
for (const [source, entry] of sources) {
  const eventHandled = isEventSource(source);
  const objectHandled = declared.has(source);
  if (entry.destination === 'observation' && !objectHandled) {
    // Either an exception, or a source that should have an object adapter.
    if (!sourceExceptions[source]) mismatch.push(`${source} 声明为 observation，但没有对象适配器`);
  }
  if (entry.destination === 'action_evidence' && !eventHandled && !declared.has(source) && !sourceExceptions[source]) {
    mismatch.push(`${source} 声明为 action_evidence，但既不由事件适配器接管，也没有例外原因`);
  }
}
check(mismatch.length === 0, `来源去向与实际处理方式不一致：${mismatch.join('、')}`);

// ---------- every type is reachable, or explicitly not wired to a source ----------
const reachable = new Set(declaredTypes);
// A source that decides its type per item names none in its declaration; its types come from the
// inventory, which is where the mapping lives.
for (const [source, entry] of sources) {
  if (declared.has(source) || isEventSource(source)) entry.types.forEach((type) => reachable.add(type));
}
const unreachable = catalog.types.map((type) => type.id).filter((id) => !reachable.has(id) && !typeExceptions[id]);
check(unreachable.length === 0, `以下类型既没有被任何已处理来源覆盖，也没有在例外表里标记未接入：${unreachable.join('、')}`);

// ---------- the exception table itself stays honest ----------
for (const source of Object.keys(sourceExceptions)) {
  check(sources.has(source), `例外表里的来源 ${source} 在来源映射里不存在——条目已过期`);
  // An exception for something that is in fact covered is worse than a missing one: it reads as a
  // known gap while the thing works, and it hides a source that later stops being covered.
  check(!declared.has(source) && !isEventSource(source), `例外表里的来源 ${source} 实际已被适配器覆盖——条目冗余`);
}
for (const type of Object.keys(typeExceptions)) {
  check(catalog.types.some((entry) => entry.id === type), `例外表里的类型 ${type} 在目录里不存在——条目已过期`);
}

// ---------- report ----------
const counts = { declared: declared.size, sources: sources.size, types: catalog.types.length, reachable: reachable.size };
console.log(`Sources: ${counts.sources} in the inventory, ${counts.declared} with declared adapters, ${[...sources.keys()].filter(isEventSource).length} by the event rule`);
console.log(`Exceptions: ${Object.keys(sourceExceptions).length} sources, ${Object.keys(typeExceptions).length} types not wired to any source`);
console.log(`Types: ${counts.reachable} of ${counts.types} reachable from a handled source`);

if (failures.length) {
  console.error(`\nAdapter coverage FAILED (${failures.length}):`);
  for (const failure of failures) console.error(`  ! ${failure}`);
  process.exit(1);
}
console.log('\nAdapter coverage verified.');
