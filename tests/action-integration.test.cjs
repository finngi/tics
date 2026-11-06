'use strict';
const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const http = require('node:http');
const { spawn, spawnSync } = require('node:child_process');

const binary = process.env.TICS_TEST_BINARY;
const skip = !binary ? 'Set TICS_TEST_BINARY to a compiled tics executable' : !fs.existsSync(binary) ? `TICS_TEST_BINARY does not exist: ${binary}` : false;

test('action entry point scans a checkout and creates, updates, or falls back from comments', { skip }, async () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'tics-action-integration-'));
  const git = (...args) => {
    const result = spawnSync('git', args, { cwd: root, encoding: 'utf8' });
    assert.equal(result.status, 0, result.stderr);
    return result.stdout.trim();
  };
  let mode = 'create';
  let sha;
  const writes = [];
  const server = http.createServer((req, res) => {
    res.setHeader('content-type', 'application/json');
    const url = new URL(req.url, 'http://localhost');
    if (url.pathname.endsWith('/pulls/17/files')) return res.end(JSON.stringify([{ filename: 'draft.md', status: 'modified' }]));
    if (url.pathname.endsWith('/pulls/17')) return res.end(JSON.stringify({ head: { sha } }));
    if (url.pathname.endsWith('/issues/17/comments') && req.method === 'GET') {
      if (mode === 'readonly') { res.statusCode = 403; return res.end('{}'); }
      return res.end(JSON.stringify(mode === 'update' ? [{ id: 123, user: { login: 'github-actions[bot]' }, body: '<!-- tics-editorial-review -->' }] : []));
    }
    if (req.method === 'POST' || req.method === 'PATCH') {
      let body = '';
      req.on('data', chunk => { body += chunk; });
      req.on('end', () => { writes.push({ method: req.method, url: url.pathname, body: JSON.parse(body).body }); res.end('{}'); });
      return;
    }
    res.statusCode = 404; res.end('{}');
  });
  try {
    git('init', '-q');
    git('config', 'user.email', 'test@example.invalid');
    git('config', 'user.name', 'Test');
    fs.writeFileSync(path.join(root, 'draft.md'), 'It is important to note that these world-class exports are available.\n');
    git('add', 'draft.md'); git('commit', '-qm', 'test'); sha = git('rev-parse', 'HEAD');
    const eventPath = path.join(root, 'event.json');
    fs.writeFileSync(eventPath, JSON.stringify({ pull_request: { number: 17, head: { sha } } }));
    await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
    const env = { ...process.env, GITHUB_EVENT_NAME: 'pull_request', GITHUB_EVENT_PATH: eventPath, GITHUB_REPOSITORY: 'owner/repo', GITHUB_WORKSPACE: root, GITHUB_API_URL: `http://127.0.0.1:${server.address().port}`, GITHUB_SERVER_URL: 'https://github.com', GITHUB_STEP_SUMMARY: path.join(root, 'summary.md'), GITHUB_OUTPUT: path.join(root, 'output.txt'), RUNNER_TEMP: root, TICS_TOKEN: 'test', TICS_BINARY: path.resolve(binary), TICS_CONFIG: '', TICS_SELECT: '', TICS_IGNORE: '', TICS_SEVERITY: '' };
    const run = () => new Promise((resolve, reject) => {
      fs.writeFileSync(env.GITHUB_OUTPUT, '');
      const child = spawn(process.execPath, [path.join(__dirname, '../action/run.cjs')], { env });
      let output = ''; child.stdout.on('data', chunk => { output += chunk; }); child.stderr.on('data', chunk => { output += chunk; });
      child.on('error', reject); child.on('close', code => resolve({ code, output }));
    });
    const outputs = () => Object.fromEntries(fs.readFileSync(env.GITHUB_OUTPUT, 'utf8').trim().split('\n').filter(Boolean).map(line => { const i = line.indexOf('='); return [line.slice(0, i), line.slice(i + 1)]; }));
    for (mode of ['create', 'update', 'readonly']) {
      fs.writeFileSync(env.GITHUB_STEP_SUMMARY, '');
      const result = await run();
      assert.equal(result.code, 0, result.output);
      const summary = fs.readFileSync(env.GITHUB_STEP_SUMMARY, 'utf8');
      assert.match(summary, /matches? across 1 of 1 file/);
      assert.match(summary, /draft\.md/);
      assert.match(summary, /<details>/);
      assert.match(summary, /world-class exports/);
      const values = outputs();
      const report = JSON.parse(fs.readFileSync(values['report-path'], 'utf8'));
      assert.equal(report.schema, 'tics.review.v1');
      assert.equal(Number(values.candidates), report.summary.candidates);
      assert.equal(Number(values['files-needing-review']), report.summary.files_needing_review);
      assert.ok(report.summary.candidates > 0);
      assert.equal(report.files[0].source, fs.readFileSync(path.join(root, 'draft.md'), 'utf8'));
      const candidate = report.files[0].candidates[0];
      assert.equal(Buffer.from(report.files[0].source).subarray(candidate.range.start_byte, candidate.range.end_byte).toString(), candidate.original);
      assert.match(summary, /\/blob\/[a-f0-9]{40}\/draft\.md#L1/);
      assert.doesNotMatch(summary, /unflagged|not been reviewed/);
      assert.ok(!summary.includes(root));
      if (mode === 'readonly') assert.match(result.output, /job summary/);
    }
    assert.deepEqual(writes.map(x => x.method), ['POST', 'PATCH']);
    assert.match(writes[0].body, /<!-- tics-editorial-review -->/);
    assert.match(writes[0].body, /matches? across 1 of 1 file/);
    assert.match(writes[0].body, /draft\.md/);
    assert.ok(!writes[0].body.includes(root));
    assert.match(writes[1].url, /issues\/comments\/123$/);
    fs.writeFileSync(path.join(root, '.tics.json'), JSON.stringify({ custom_rules: [{ id: 'team-marketing', title: 'Marketing wording', pattern: 'world-class', hint: 'Give a specific comparison.', severity: 'warn' }] }));
    mode = 'update'; env.TICS_CONFIG = '.tics.json'; env.TICS_SELECT = 'team-marketing';
    assert.equal((await run()).code, 0);
    assert.match(writes.at(-1).body, /Marketing wording/);
    assert.match(writes.at(-1).body, /1 match across 1 of 1 file/);
    env.TICS_IGNORE = 'team-marketing';
    assert.equal((await run()).code, 0);
    assert.match(writes.at(-1).body, /No selected patterns matched/);
    const clean = outputs();
    assert.equal(clean.candidates, '0');
    assert.equal(clean['files-needing-review'], '0');
    assert.equal(JSON.parse(fs.readFileSync(clean['report-path'], 'utf8')).summary.candidates, 0);
    env.TICS_IGNORE = 'missing-rule';
    const bad = await run();
    assert.notEqual(bad.code, 0); assert.match(bad.output, /unknown rule id/);
    assert.equal(fs.readFileSync(env.GITHUB_OUTPUT, 'utf8'), '');
    env.TICS_IGNORE = '';
    fs.writeFileSync(eventPath, JSON.stringify({ pull_request: { number: 17, head: { sha: 'a'.repeat(40) } } }));
    const stale = await run();
    assert.notEqual(stale.code, 0); assert.match(stale.output, /Checkout must be/);
  } finally {
    await new Promise(resolve => server.close(resolve));
    fs.rmSync(root, { recursive: true, force: true });
  }
});
