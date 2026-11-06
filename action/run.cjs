'use strict';
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const { renderReport } = require('./render.cjs');

const MARKER = '<!-- tics-editorial-review -->';
const COMMENT_LIMIT = 60000;
const SUMMARY_LIMIT = 900000;

async function request(url, token, options = {}) {
  const response = await fetch(url, {
    ...options,
    headers: { accept: 'application/vnd.github+json', authorization: `Bearer ${token}`,
      'x-github-api-version': '2022-11-28', ...(options.body ? { 'content-type': 'application/json' } : {}) },
  });
  if (!response.ok) {
    const error = new Error(`GitHub API ${response.status}: ${url}`);
    error.status = response.status;
    throw error;
  }
  return { data: await response.json(), link: response.headers.get('link') };
}

async function pages(url, token, api = request) {
  const results = [];
  while (url) {
    const page = await api(url, token);
    if (!Array.isArray(page.data)) throw new Error('Expected an array from GitHub API');
    results.push(...page.data);
    const match = page.link && page.link.match(/<([^>]+)>;\s*rel="next"/);
    url = match ? match[1] : null;
  }
  return results;
}

function checkedFile(name, workspace) {
  const root = fs.realpathSync(workspace);
  if (typeof name !== 'string' || name.includes('\0') || path.isAbsolute(name) || name.split('/').some(p => !p || p === '.' || p === '..')) throw new Error(`Invalid repository path: ${name}`);
  let current = root;
  for (const part of name.split('/')) {
    current = path.join(current, part);
    if (fs.lstatSync(current).isSymbolicLink()) throw new Error(`Refusing symlink path: ${name}`);
  }
  if (!fs.statSync(current).isFile()) throw new Error(`Not a regular file: ${name}`);
  return name;
}

function eligibleFiles(files, workspace) {
  return files.filter(file => file.status !== 'removed' && /\.(?:md|markdown|txt)$/i.test(file.filename))
    .map(file => checkedFile(file.filename, workspace));
}

function reviewArgs(settings, workspace) {
  const args = [];
  if (settings.config?.trim()) args.push('--config', checkedFile(settings.config.trim(), workspace));
  for (const name of ['select', 'ignore']) {
    const ids = (settings[name] || '').split(/[,\s]+/).filter(Boolean);
    if (ids.length) args.push(`--${name}`, ids.join(','));
  }
  const severity = settings.severity?.trim();
  if (severity) {
    if (!['info', 'warn'].includes(severity)) throw new Error('severity must be info or warn');
    args.push('--severity', severity);
  }
  return args;
}

function scan(binary, files, workspace, settings = {}, execute = spawnSync) {
  const args = ['review', '--json', ...reviewArgs(settings, workspace), '--', ...(files.length ? files : ['-'])];
  const result = execute(binary, args, { cwd: workspace, input: '', encoding: 'utf8', maxBuffer: 16 * 1024 * 1024 });
  if (result.error || (result.status !== 0 && result.status !== 1)) throw new Error(`tics scan failed: ${result.error || result.stderr || `exit ${result.status}`}`);
  let report;
  try { report = JSON.parse(result.stdout); } catch { throw new Error('tics returned invalid JSON'); }
  if (report.schema !== 'tics.review.v1' || !Array.isArray(report.files)) throw new Error('tics returned an unsupported review report');
  // An empty stdin scan still validates configuration when the PR changes no prose.
  if (!files.length) {
    report.files = [];
    for (const name of Object.keys(report.summary)) report.summary[name] = 0;
  }
  return report;
}

async function publish(base, number, token, markdown, api = request, list = pages) {
  const url = `${base}/issues/${number}/comments`;
  const body = `${MARKER}\n${markdown}`;
  if (Buffer.byteLength(body) > COMMENT_LIMIT) throw new Error('PR comment exceeds its rendered size limit');
  try {
    const comments = await list(`${url}?per_page=100`, token, api);
    const previous = comments.find(c => c.user?.login === 'github-actions[bot]' && typeof c.body === 'string' && c.body.includes(MARKER));
    await api(previous ? `${base}/issues/comments/${previous.id}` : url, token,
      { method: previous ? 'PATCH' : 'POST', body: JSON.stringify({ body }) });
    return true;
  } catch (error) {
    if (error.status === 403 || error.status === 404) return false;
    throw error;
  }
}

async function main() {
  if (process.env.GITHUB_EVENT_NAME !== 'pull_request') throw new Error('This action requires a pull_request event');
  const event = JSON.parse(fs.readFileSync(process.env.GITHUB_EVENT_PATH, 'utf8'));
  const number = event.pull_request?.number;
  const sha = event.pull_request?.head?.sha;
  if (!Number.isSafeInteger(number) || number < 1 || !/^[a-f0-9]{40}$/i.test(sha || '')) throw new Error('Invalid pull request event');
  const repo = process.env.GITHUB_REPOSITORY;
  if (!/^[\w.-]+\/[\w.-]+$/.test(repo || '')) throw new Error('Invalid repository');
  const token = process.env.TICS_TOKEN;
  if (!token) throw new Error('A GitHub token is required');
  const workspace = process.env.GITHUB_WORKSPACE;
  const head = spawnSync('git', ['rev-parse', 'HEAD'], { cwd: workspace, encoding: 'utf8' });
  if (head.status !== 0 || head.stdout.trim() !== sha) throw new Error('Checkout must be the pull request head SHA');
  const base = `${process.env.GITHUB_API_URL}/repos/${repo}`;
  const pullUrl = `${base}/pulls/${number}`;
  if ((await request(pullUrl, token)).data.head?.sha !== sha) throw new Error('Pull request head changed; rerun on the current commit');
  const changed = await pages(`${pullUrl}/files?per_page=100`, token);
  if ((await request(pullUrl, token)).data.head?.sha !== sha) throw new Error('Pull request head changed while listing files');
  if (changed.length >= 3000) throw new Error('Pull request file list may be truncated at 3000 files');
  const settings = { config: process.env.TICS_CONFIG, select: process.env.TICS_SELECT, ignore: process.env.TICS_IGNORE, severity: process.env.TICS_SEVERITY };
  const report = scan(process.env.TICS_BINARY, eligibleFiles(changed, workspace), workspace, settings);
  const reportDir = fs.mkdtempSync(path.join(process.env.RUNNER_TEMP || os.tmpdir(), 'tics-report-'));
  const reportPath = path.join(reportDir, 'review.json');
  fs.writeFileSync(reportPath, JSON.stringify(report, null, 2) + '\n');
  fs.appendFileSync(process.env.GITHUB_OUTPUT, `report-path=${reportPath}\ncandidates=${report.summary.candidates}\nfiles-needing-review=${report.summary.files_needing_review}\n`);
  const options = { serverUrl: process.env.GITHUB_SERVER_URL || 'https://github.com', repository: repo, sha };
  fs.appendFileSync(process.env.GITHUB_STEP_SUMMARY, renderReport(report, { ...options, limit: SUMMARY_LIMIT }) + '\n');
  const posted = await publish(base, number, token, renderReport(report, { ...options, limit: COMMENT_LIMIT - Buffer.byteLength(MARKER + '\n') }));
  if (!posted) console.log('Comment unavailable with this token; report is in the job summary.');
}

module.exports = { checkedFile, eligibleFiles, pages, publish, reviewArgs, scan };
if (require.main === module) main().catch(error => { console.error(error.message); process.exitCode = 1; });
