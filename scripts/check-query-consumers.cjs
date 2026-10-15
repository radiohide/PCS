'use strict';
// S26: only one module reads the query contract's response fields, and it declares them.
//
// The point of "no self-defined DTO" is not that pages may not show anything — it is that a field
// read from a response has to be declared somewhere a check can verify against the contract.
// `model-view.js` declares every path it reads, and `check-model-view.cjs` resolves each one against
// the fixed response set.
//
// A page that formats a response field itself has quietly built a second view of the contract: it
// keeps compiling and keeps rendering when the field is renamed, and it renders roughly the right
// thing from the wrong field. So the rule here is narrow and mechanical:
//
//   * every UI module that sends a model query must be listed with its role;
//   * outside the renderer, a response may be read for `status` — control flow, not presentation —
//     and for `page`, which is how a caller walks a result. Everything else goes through the
//     renderer.
const fs = require('node:fs');
const path = require('node:path');
const root = path.resolve(__dirname, '..');
const ui = path.join(root, 'apps/desktop/ui');

// The modules that may send a model query, and what they are allowed to be doing with the answer.
const CONSUMERS = {
  'app.js': 'router: owns the page walk (status, page) and hands every response to the renderer',
  'settings.js': 'router: loads the migration state lazily when its section is opened, and renders it through the renderer',
  'object-tree.js': 'router: walks filtered object branches and uses model-view treePage and detail presentation',
};
const RENDERER = 'model-view.js';
// Response fields that must never be read outside the renderer. `status` and `page` are absent from
// this list on purpose: one is control flow and the other is the walk.
const FORBIDDEN = ['counts', 'data', 'warnings', 'fields', 'summary', 'actions_preview', 'bindings', 'catalog_version', 'contract_version', 'mock'];

// The old object-reading operations. The model replaced every one of them, and a page that went back
// to one would be reading the old source-and-object list while the rest of the interface reads the
// model. S33 removed the variants from the IPC enum on 2026-09-12, so these can no longer be served
// at all; what is checked now is that no page still sends one, and that the enum really is rid of
// them — a list here that quietly outlived the operations it names would be the wrong kind of green.
const RETIRED = ['timeline', 'timeline_search', 'objects', 'object_search', 'snapshots', 'compare', 'task_evidence', 'event_related'];
// Operations that are configuration rather than object presentation, and stay until their own step:
// the plugin switches, collection items, retention, privacy and the file watch list.
const CONFIGURATION = ['status', 'plugins', 'set_plugin', 'collect_plugin', 'collect', 'item_controls', 'set_item_enabled', 'retention', 'set_retention', 'privacy_settings', 'set_privacy_settings', 'file_watch_settings', 'set_file_watch_settings', 'directory_watch_settings', 'set_directory_watch_settings', 'runtime_watch_settings', 'set_runtime_watch_settings'];

// `timeline_search` -> `TimelineSearch`, the enum variant `#[serde(rename_all="snake_case")]` maps it
// back from.
const variantOf = (operation) => operation.split('_').map((part) => part[0].toUpperCase() + part.slice(1)).join('');

const failures = [];

// Both spellings a page can use. The literal `api('timeline'` form alone was a blind spot: the
// inline `operation:'timeline'` form appears in the UI too, and used to slip past this check
// entirely — a green result for a call the check was written to find.
const calls = (source, operation) => new RegExp(`(?:api\\(\\s*['"]${operation}['"]|operation\\s*:\\s*['"]${operation}['"])`).test(source);
for (const entry of fs.readdirSync(ui)) {
  if (!entry.endsWith('.js')) continue;
  const source = fs.readFileSync(path.join(ui, entry), 'utf8');
  for (const operation of RETIRED) {
    if (calls(source, operation)) failures.push(`${entry} 仍在调用已由对象模型取代的旧操作 ${operation}`);
  }
}

// The operations are gone from the enum, not merely unused by the frontend.
const ipc = fs.readFileSync(path.join(root, 'crates/ipc/src/lib.rs'), 'utf8');
const enumStart = ipc.indexOf('pub enum Request {');
const enumBody = enumStart === -1 ? '' : ipc.slice(enumStart, ipc.indexOf('\n}', enumStart));
// Without this, a moved or renamed enum would leave `enumBody` empty and every absence check below
// would pass for the wrong reason — the failure mode this whole script exists to catch.
if (!/\bQuery\b/.test(enumBody)) {
  failures.push('未能定位 crates/ipc/src/lib.rs 的 Request 枚举：下面的「变体已删除」断言将失去意义');
} else {
  for (const operation of RETIRED) {
    const variant = variantOf(operation);
    if (new RegExp(`\\b${variant}\\b`).test(enumBody)) failures.push(`crates/ipc/src/lib.rs 的 Request 枚举仍声明着 ${variant}（线上串 ${operation}）`);
  }
}
for (const entry of fs.readdirSync(ui)) {
  if (!entry.endsWith('.js') || entry === RENDERER) continue;
  const source = fs.readFileSync(path.join(ui, entry), 'utf8');
  if (!source.includes("api('query'")) continue;
  if (!CONSUMERS[entry]) {
    failures.push(`${entry} 发送了模型查询，但未在 CONSUMERS 中登记其角色`);
    continue;
  }
  for (const field of FORBIDDEN) {
    if (source.includes(`response.${field}`)) {
      failures.push(`${entry} 直接读取了 response.${field}——该字段应当交给 ${RENDERER} 渲染`);
    }
  }
}
// A check that passes because nothing consumes the contract would be worse than none.
const consumers = Object.keys(CONSUMERS);
if (consumers.length === 0) failures.push('CONSUMERS 为空：没有任何页面在消费查询契约，检查无意义');

console.log(`Query consumers: ${consumers.length} page(s) routing responses, renderer ${RENDERER}`);
console.log(`Retired object operations: ${RETIRED.length} checked — absent from the UI and from the Request enum; ${CONFIGURATION.length} configuration operations still in place`);
if (failures.length) {
  console.error(`\nQuery consumers FAILED (${failures.length}):`);
  for (const failure of failures) console.error(`  ! ${failure}`);
  process.exit(1);
}
console.log('Query consumers verified.');
