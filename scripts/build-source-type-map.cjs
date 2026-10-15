'use strict';
// What each source provides: which object types it feeds, and what those types declare.
//
// The plan asks the plugins page to keep every switch as it is and add the mapping explanation
// beside it — which objects, fields and actions a source contributes. That is a join of three
// things the repository already holds, and joining them by hand is how the page would drift from
// them:
//
//   * `model/source-map.cjs`       — which types a source is mapped to, and where its material goes
//   * `model/catalog.json`         — what those types declare as fields and actions
//   * `model/adapter-coverage.json`— whether a source has an adapter yet
//
// usage: node scripts/build-source-type-map.cjs [--check]
const fs = require('node:fs');
const path = require('node:path');
const root = path.resolve(__dirname, '..');
const p = (...parts) => path.join(root, ...parts);

const checkOnly = process.argv.includes('--check');

const catalog = JSON.parse(fs.readFileSync(p('model/catalog.json'), 'utf8'));
const coverage = JSON.parse(fs.readFileSync(p('model/adapter-coverage.json'), 'utf8'));
const { rows } = require(p('model/source-map.cjs'));

const byType = new Map(catalog.types.map((type) => [type.id, type]));
const declared = new Set(coverage.declared.map((entry) => entry.source));
const isEventSource = (source) =>
  !coverage.event_exclusions.includes(source) &&
  ((coverage.event_sources || []).includes(source) || source.startsWith(coverage.event_prefix) || source.startsWith(coverage.usn_prefix));

const sources = new Map();
for (const row of rows) {
  const [source, destination, types] = row.split('|');
  if (!sources.has(source)) sources.set(source, { source, destination, types: [] });
  for (const type_id of (types || '').split(',').map((t) => t.trim()).filter((t) => t && t !== '-')) {
    sources.get(source).types.push(type_id);
  }
}

const document = {
  generated_by: 'scripts/build-source-type-map.cjs',
  note: '每个来源提供哪些对象类型，以及那些类型声明了哪些字段与动作。由来源映射、目录与适配器覆盖三处连接而成；不手工维护。',
  sources: [...sources.values()]
    .sort((left, right) => left.source.localeCompare(right.source))
    .map((entry) => ({
      source: entry.source,
      destination: entry.destination,
      // How this source's material is handled: an object adapter, the shared event adapter, or
      // nothing yet. A source with no path is shown as such rather than as an empty mapping.
      handling: declared.has(entry.source)
        ? (['coverage','sampling'].includes(entry.destination) ? 'source_reading' : entry.destination==='action_evidence' ? 'action_adapter' : 'object_adapter')
        : isEventSource(entry.source) ? 'event_adapter' : entry.source==='pcscope.usn.changes' ? 'virtual_group' : 'unmapped',
      types: entry.types.map((type_id) => {
        const type = byType.get(type_id);
        return {
          type_id,
          // A type named in the map but absent from the catalog is a mapping error; it is reported
          // rather than dropped, because a silently missing type reads as one that provides nothing.
          name: type ? type.name : '(未在目录中声明)',
          category: type ? type.category : '',
          fields: type ? type.fields.map((field) => field.id) : [],
          actions: type ? (type.actions || []) : [],
        };
      }),
    })),
};
const text = JSON.stringify(document, null, 2) + '\n';
const target = p('apps/desktop/ui/source-type-map.json');

if (checkOnly) {
  const current = fs.existsSync(target) ? fs.readFileSync(target, 'utf8') : '';
  if (current !== text) {
    console.error('source type map is out of date; run: node scripts/build-source-type-map.cjs');
    process.exit(1);
  }
  console.log(`Source type map verified: ${document.sources.length} sources, ${document.sources.filter((s) => s.handling === 'unmapped').length} with no handling path`);
} else {
  fs.writeFileSync(target, text);
  const unmapped = document.sources.filter((entry) => entry.handling === 'unmapped');
  console.log(`Source type map written: ${document.sources.length} sources, ${document.sources.filter((s) => s.handling === 'object_adapter').length} with object adapters, ${document.sources.filter((s) => s.handling === 'event_adapter').length} by the event rule, ${unmapped.length} unmapped`);
  for (const entry of unmapped) console.log(`  unmapped: ${entry.source}（${entry.destination}）`);
}
