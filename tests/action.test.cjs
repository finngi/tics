'use strict';
const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { eligibleFiles, pages, publish, reviewArgs, scan } = require('../action/run.cjs');

test('follows API pagination', async () => {
  const api = async url => url === 'first' ? { data: [1], link: '<second>; rel="next"' } : { data: [2], link: null };
  assert.deepEqual(await pages('first', 'token', api), [1, 2]);
});

test('creates and updates the bot comment, with read-only fallback', async () => {
  const calls = [];
  const api = async (url, token, options) => { calls.push({ url, options }); return { data: {} }; };
  assert.equal(await publish('base', 7, 'token', 'report', api, async () => []), true);
  assert.equal(calls[0].options.method, 'POST');
  await publish('base', 7, 'token', 'report', api, async () => [{ id: 9, user: { login: 'github-actions[bot]' }, body: '<!-- tics-editorial-review -->' }]);
  assert.equal(calls[1].options.method, 'PATCH');
  assert.equal(calls[1].url, 'base/issues/comments/9');
  assert.equal(await publish('base', 7, 'token', 'report', api, async () => { throw Object.assign(new Error('denied'), { status: 403 }); }), false);
  await assert.rejects(publish('base', 7, 'token', 'report', api, async () => { throw Object.assign(new Error('server error'), { status: 500 }); }));
});

test('scan handles candidate exit separately from errors', () => {
  const report = { schema: 'tics.review.v1', summary: { files: 1, candidates: 1 }, files: [{ path: 'a.md' }] };
  for (const status of [0, 1]) assert.deepEqual(scan('tics', ['a.md'], '/workspace', {}, () => ({ status, stdout: JSON.stringify(report) })), report);
  assert.throws(() => scan('tics', ['a.md'], '/workspace', {}, () => ({ status: 2, stderr: 'bad input' })), /bad input/);
  assert.throws(() => scan('tics', ['a.md'], '/workspace', {}, () => ({ status: 0, stdout: 'bad JSON' })), /invalid JSON/);
  const empty = scan('tics', [], '/workspace', {}, (_binary, args) => {
    assert.equal(args.at(-1), '-');
    return { status: 0, stdout: JSON.stringify(report) };
  });
  assert.deepEqual(empty.files, []);
  assert.equal(empty.summary.files, 0);
  assert.throws(() => scan('tics', [], '/workspace', {}, () => ({ status: 2, stderr: 'unknown rule' })), /unknown rule/);
});

test('selects prose and rejects traversal and symlinks', () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'tics-action-'));
  try {
    fs.writeFileSync(path.join(root, 'a.md'), 'text');
    fs.symlinkSync(os.tmpdir(), path.join(root, 'link'));
    assert.deepEqual(eligibleFiles([{ filename: 'a.md', status: 'modified' }, { filename: 'a.js', status: 'modified' }, { filename: 'gone.txt', status: 'removed' }], root), ['a.md']);
    assert.throws(() => eligibleFiles([{ filename: '../x.md', status: 'added' }], root), /Invalid/);
    assert.throws(() => eligibleFiles([{ filename: 'link/x.md', status: 'added' }], root), /symlink/);
  } finally { fs.rmSync(root, { recursive: true, force: true }); }
});

test('forwards rule inputs as arguments and validates the config path', () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'tics-action-config-'));
  try {
    fs.writeFileSync(path.join(root, '.tics.json'), '{}');
    fs.symlinkSync(path.join(root, '.tics.json'), path.join(root, 'link.json'));
    assert.deepEqual(reviewArgs({ config: '.tics.json', select: 'ai-vocabulary,\nsummary-opener', ignore: 'em-dash', severity: 'warn' }, root), ['--config', '.tics.json', '--select', 'ai-vocabulary,summary-opener', '--ignore', 'em-dash', '--severity', 'warn']);
    assert.throws(() => reviewArgs({ config: '../outside.json' }, root), /Invalid/);
    assert.throws(() => reviewArgs({ config: 'link.json' }, root), /symlink/);
    assert.throws(() => reviewArgs({ severity: 'maybe' }, root), /severity/);
  } finally { fs.rmSync(root, { recursive: true, force: true }); }
});
