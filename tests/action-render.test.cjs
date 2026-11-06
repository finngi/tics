'use strict';
const { test } = require('node:test');
const assert = require('node:assert/strict');
const { renderReport } = require('../action/render.cjs');

const opts = { serverUrl: 'https://github.com', repository: 'owner/repo', sha: 'abc123', limit: 60000 };
function sample() {
  const source = '\uFEFFIntro\r\n\r\nCafé ``` context.\r\n';
  const start = Buffer.byteLength('\uFEFFIntro\r\n\r\n');
  const end = Buffer.byteLength(source);
  const range = { start_byte: start, end_byte: end, start_line: 3 };
  const candidate = { id: 'f1', passage_id: 'p1', title: 'Check wording', rule: 'some-rule', range, original: 'Café ``` context.', review_hint: 'Is this useful?' };
  return { schema: 'tics.review.v1', files: [{ path: 'docs/my file.md', source, candidates: [candidate], passages: [{ id: 'p1', range, candidate_count: 1 }] }, { path: 'clear.txt', candidates: [], passages: [{ id: 'p1', range: { start_byte: 0, end_byte: 10 }, candidate_count: 0 }] }] };
}

test('renders file counts, source context, and complete collapsed details', () => {
  const text = renderReport(sample(), opts);
  assert.doesNotMatch(text, /^\|/m);
  assert.match(text, /\*\* · 1 match\n\n\*\*\[clear\.txt\]/);
  assert.match(text, /docs\/my%20file\.md#L3/);
  assert.match(text, /0 matches\n\n/);
  assert.match(text, /````text\nCafé ``` context\.\r\n\n````/);
  assert.match(text, /\*\*Check wording\*\*/);
  assert.match(text, /Is this useful\?/);
  assert.equal((text.match(/<details>/g) || []).length, (text.match(/<\/details>/g) || []).length);
  assert.doesNotMatch(text, /LLM|authorship|Start → end|Each cell covers|[░▒▓█]/);
});

test('caps file listing and omits whole details while respecting byte limit', () => {
  const report = sample();
  report.files.push(...Array.from({ length: 70 }, (_, i) => ({ path: `file${i}.md`, candidates: [], passages: [] })));
  report.files[0].source += 'x'.repeat(10000);
  report.files[0].passages[0].range.end_byte += 10000;
  const text = renderReport(report, { ...opts, limit: 3000 });
  assert.ok(Buffer.byteLength(text) <= 3000);
  assert.match(text, /Omitted: \d+ file list rows/);
  assert.doesNotMatch(text, /x{100}/);
  assert.equal((text.match(/<details>/g) || []).length, (text.match(/<\/details>/g) || []).length);
});


test('retains complete passage groups from a large file and reports omitted groups', () => {
  const report = sample();
  const file = report.files[0];
  const small = file.passages[0];
  const original = file.candidates[0];
  const extra = 'Z'.repeat(10000);
  const start = Buffer.byteLength(file.source);
  file.source += extra;
  const range = { start_byte: start, end_byte: start + extra.length, start_line: 4 };
  file.passages.push({ id: 'p2', range, candidate_count: 1 });
  file.candidates.push({ ...original, id: 'f2', passage_id: 'p2', range, original: extra });
  const text = renderReport(report, { ...opts, limit: 2500 });
  assert.match(text, /2 matches across 1 of 2 files/);
  assert.ok(Buffer.byteLength(text) <= 2500);
  assert.match(text, /Café ``` context/);
  assert.match(text, /Omitted 1 passage group \(1 match\)/);
  assert.equal((text.match(/<details>/g) || []).length, (text.match(/<\/details>/g) || []).length);
  assert.doesNotMatch(text, /Z{10}/);
  assert.equal(small.id, 'p1');
});
