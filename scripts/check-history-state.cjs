'use strict';
// The `history_state` vocabulary, held to one list.
//
// WHY THIS EXISTS. `history_state` is written by two Rust sites and read by two front-end ones, and
// nothing used to connect them. The contract declared four words; the writers used six, three of
// which were undeclared and two of which were the same fact under different names (`gap` in the
// coverage table, `blocked` in the eventlog result). Nothing failed, because no check looked: the
// health table rendered the undeclared words as their raw English codes and the monitoring page
// silently fell through to a fallback label. A vocabulary nothing enforces is a vocabulary that
// drifts.
//
// So: the declared list in `response_shape()` is the authority. Every word a writer produces must be
// in it, and every word it declares must be one the health table can name. Negative controls for
// both directions were run when this was added — see docs/refactor/shared-changes/0009.
const fs = require('node:fs');
const path = require('node:path');
const root = path.resolve(__dirname, '..');
// Normalised to LF: this repository checks out with core.autocrlf, so a pattern anchored on a line
// ending would otherwise match in one work tree and not in another.
const read = (...parts) => fs.readFileSync(path.join(root, ...parts), 'utf8').replace(/\r\n/g, '\n');

const failures = [];
const check = (condition, message) => { if (!condition) failures.push(message); };

// Rust string literals: double-quoted, lowercase snake case.
const rustWords = (text) => [...text.matchAll(/"([a-z_]+)"/g)].map((match) => match[1]);
// JavaScript object keys: the name before the colon, quoted with either kind of quote.
const jsKeys = (text) => [...text.matchAll(/(?:^|[{,\s])['"]?([a-z_]+)['"]?\s*:/g)].map((match) => match[1]);
// The field name itself is not one of its own values.
const FIELD = 'history_state';
const words = (text) => rustWords(text).filter((word) => word !== FIELD);

// ---------- the authority: what the contract declares ----------
const contract = read('crates/contracts/src/query.rs');
const declaredRegion = contract.match(/"coverage_history"\s*:\s*\[([^\]]*)\]/);
const declared = declaredRegion ? rustWords(declaredRegion[1]) : [];
check(declared.length > 0,
  '未能从 crates/contracts/src/query.rs 的 response_shape() 读到 coverage_history：词表检查失去基准');

// The generated mirror must agree with the declaration, or the front-end is checked against a
// different list than the one the contract publishes.
const samples = JSON.parse(read('model/query-samples.json'));
const mirrored = samples.shape ? samples.shape.coverage_history : [];
check(Array.isArray(mirrored) && mirrored.join(',') === declared.join(','),
  `model/query-samples.json 的 coverage_history（${JSON.stringify(mirrored)}）与契约声明（${JSON.stringify(declared)}）不一致；` +
  '该文件由 crates/contracts/examples/query-contract.rs 生成，请重新生成而不是手改');

// ---------- the writers: every word they produce must be declared ----------
// Each rule captures the region holding one site's values rather than scraping the whole file — a
// bare scan would pull in unrelated literals and turn this into a check that cries wolf.
const WRITERS = [
  ['crates/storage/src/coverage.rs', /let history_state=([\s\S]*?);\n/, 'coverage.rs 的 history_state 赋值'],
  ['crates/service/src/main.rs', /"history_state":([^\n]*)/, 'service/main.rs 的事件日志结果'],
  ['crates/storage/src/flow.rs', /row\["history_state"\][^\n]*?(unwrap_or\([^)]*\))/, 'flow.rs 的兜底值'],
];
const written = new Map();
for (const [file, pattern, label] of WRITERS) {
  const region = read(file).match(pattern);
  check(region !== null, `未能定位 ${label}（${file}）：下面的「写入词已声明」断言对该站点失去意义`);
  if (!region) continue;
  const site = words(region[1]);
  check(site.length > 0, `${label} 没有解析出任何词：规则可能已过期，这条断言正在空转`);
  written.set(file, site);
  for (const word of site) {
    check(declared.includes(word), `${label} 写出了词表外的 ${word}——契约只声明 ${declared.join(' / ')}`);
  }
}

// ---------- the readers: every declared word must have a name ----------
// The health table renders whatever the coverage table holds, so it has to name all of them.
const names = read('apps/desktop/ui/model-view.js').match(/const historyNames=\{([^}]*)\}/);
check(names !== null, 'model-view.js 中找不到 historyNames：健康表可能已改用别的方式取名');
if (names) {
  const labelled = jsKeys(names[1]);
  check(labelled.length > 0, 'historyNames 没有解析出任何键：规则可能已过期');
  for (const word of declared) {
    check(labelled.includes(word), `model-view.js 的 historyNames 没有为 ${word} 取名，健康表会直接显示英文码`);
  }
}
// The monitoring page renders the eventlog result, so it has to name the words that writer emits.
const monitoring = read('apps/desktop/ui/monitoring.js');
const channelWords = written.get('crates/service/src/main.rs') || [];
for (const word of channelWords) {
  check(new RegExp(`\\b${word}\\s*:`).test(monitoring),
    `monitoring.js 没有为 ${word} 取名，采集权限页会落到兜底文案`);
}

console.log(`History state: ${declared.length} declared word(s) [${declared.join(', ')}]`);
console.log(`  writers: ${[...written].map(([file, site]) => `${path.basename(file)}=${site.join('/')}`).join('  ')}`);
if (failures.length) {
  console.error(`\nHistory state FAILED (${failures.length}):`);
  for (const failure of failures) console.error(`  ! ${failure}`);
  process.exit(1);
}
console.log('History state vocabulary verified.');
