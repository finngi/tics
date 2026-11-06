'use strict';

const MAX_ROWS = 50;
const html = value => String(value).replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;').replaceAll('"', '&quot;');
const md = value => html(value).replaceAll('\\', '\\\\').replaceAll('|', '\\|').replaceAll('[', '\\[').replaceAll(']', '\\]');
const bytes = value => Buffer.byteLength(value, 'utf8');
const counted = (n, noun) => `${n} ${n === 1 ? noun : noun === 'match' ? 'matches' : `${noun}s`}`;

function link(file, line, options) {
  const encode = value => encodeURIComponent(value).replaceAll('(', '%28').replaceAll(')', '%29');
  const filename = file.path.split('/').map(encode).join('/');
  const repo = options.repository.split('/').map(encode).join('/');
  return `${options.serverUrl.replace(/\/$/, '')}/${repo}/blob/${encode(options.sha)}/${filename}#L${line || 1}`;
}

function fenced(text) {
  let max = 2;
  for (const match of text.matchAll(/`+/g)) max = Math.max(max, match[0].length);
  const fence = '`'.repeat(max + 1);
  return `${fence}text\n${text}\n${fence}\n`;
}

function excerpt(source, range) {
  const buffer = Buffer.from(source, 'utf8');
  if (!range || !Number.isInteger(range.start_byte) || !Number.isInteger(range.end_byte) || range.start_byte < 0 || range.end_byte < range.start_byte || range.end_byte > buffer.length) throw new Error('Invalid passage byte range');
  return buffer.subarray(range.start_byte, range.end_byte).toString('utf8');
}

function detail(file, options, budget) {
  const passages = new Map(file.passages.map(p => [p.id, p]));
  const groups = new Map();
  for (const candidate of file.candidates) {
    const key = candidate.passage_id && passages.has(candidate.passage_id) ? candidate.passage_id : `candidate:${candidate.id}`;
    if (!groups.has(key)) groups.set(key, []);
    groups.get(key).push(candidate);
  }
  let result = `<details>\n<summary>${html(file.path)} — ${counted(file.candidates.length, 'match')}</summary>\n\n`;
  const closing = '</details>\n\n';
  let shownGroups = 0;
  let shownMatches = 0;
  // Keep room for an accurate omission notice and both closing tags.
  const notice = (groupsLeft, matchesLeft) => `Omitted ${counted(groupsLeft, 'passage group')} (${counted(matchesLeft, 'match')}) due to display limits.\n\n`;
  for (const [key, candidates] of groups) {
    const passage = passages.get(key);
    const range = passage ? passage.range : candidates[0].range;
    const line = range.start_line || 1;
    let block = `<details>\n<summary>${passage ? 'Passage' : 'Match'} at line ${line} — ${counted(candidates.length, 'match')}</summary>\n\n`;
    block += `[Open source at line ${line}](${link(file, line, options)})\n\n`;
    block += fenced(excerpt(file.source, range)) + '\n';
    for (const candidate of candidates) {
      const title = candidate.title || candidate.rule.replaceAll(/[-_]+/g, ' ');
      block += `**${md(title)}** · [line ${candidate.range.start_line}](${link(file, candidate.range.start_line, options)})\n\n`;
      block += `${html(candidate.review_hint)}\n\nMatched text:\n\n${fenced(candidate.original)}\n`;
    }
    block += closing;
    const remaining = notice(groups.size - shownGroups - 1, file.candidates.length - shownMatches - candidates.length);
    if (bytes(result + block + remaining + closing) > budget) continue;
    result += block;
    shownGroups++;
    shownMatches += candidates.length;
  }
  if (shownGroups < groups.size) result += notice(groups.size - shownGroups, file.candidates.length - shownMatches);
  result += closing;
  return bytes(result) <= budget ? { text: result, matches: shownMatches } : null;
}

function renderReport(report, options) {
  if (report.schema !== 'tics.review.v1' || !Array.isArray(report.files)) throw new Error('Expected tics.review.v1 report');
  const { limit = 60000 } = options;
  if (!Number.isInteger(limit) || limit < 0) throw new Error('Invalid Markdown byte limit');
  const files = report.files;
  const findings = files.reduce((n, file) => n + file.candidates.length, 0);
  const matched = files.filter(file => file.candidates.length).length;
  const overview = findings ? `${counted(findings, 'match')} across ${matched} of ${counted(files.length, 'file')}.` : `No selected patterns matched across ${counted(files.length, 'file')}.`;
  let output = `## Tics\n\n${overview}\n\n`;
  // Reserve room for a complete omissions notice.
  const reserve = 300;
  if (bytes(output) + reserve > limit) {
    const fallback = `## Tics\n\n${overview} Report omitted due to size limit.\n`;
    return bytes(fallback) <= limit ? fallback : '';
  }
  let rows = 0;
  for (const file of files.slice(0, MAX_ROWS)) {
    const row = `**[${md(file.path)}](${link(file, 1, options)})** · ${counted(file.candidates.length, 'match')}\n\n`;
    if (bytes(output + row) + reserve > limit) break;
    output += row; rows++;
  }
  let displayedFiles = 0;
  let displayedMatches = 0;
  for (const file of files) {
    if (!file.candidates.length) continue;
    const block = detail(file, options, limit - bytes(output) - reserve);
    if (!block) continue;
    output += block.text;
    displayedFiles++;
    displayedMatches += file.candidates.length;
  }
  const omittedRows = files.length - rows;
  const omittedFiles = matched - displayedFiles;
  const omittedMatches = findings - displayedMatches;
  if (omittedRows || omittedFiles) output += `Omitted: ${counted(omittedRows, 'file list row')}; details for ${counted(omittedFiles, 'file')} (${counted(omittedMatches, 'match')}) due to display limits.\n`;
  return output;
}

module.exports = { renderReport };
