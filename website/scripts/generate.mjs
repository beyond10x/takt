// Writes the site's generated inputs from the repository's records, before every build. Nothing
// it writes is committed (website/.gitignore), so a page can never drift from its record:
//
//   data/status.json            b10x-status/1, from the stories in .engineering/planning/story/
//   src/generated/charts.js     docs/metrics/timeline.svg and journey.svg, as strings
//   static/protocols/           the protocol viewer, built from docs/site/protocols/
//   docs/reference/cli.md       every command's --help, from the binary built from this checkout
//   docs/reference/exit-codes.md  the exit-code sections of the same help
//
// The binary is `cargo run --locked -p b10x-takt-cli`, or TAKT_BIN when it names one.
// Run from anywhere: node website/scripts/generate.mjs. Set TAKT_SITE_SKIP_VIEWER=1 to leave the
// viewer out (the build then has no /protocols/ page).
import {execFileSync} from 'node:child_process';
import {cpSync, mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync} from 'node:fs';
import {dirname, join} from 'node:path';
import {fileURLToPath} from 'node:url';

const website = join(dirname(fileURLToPath(import.meta.url)), '..');
const repo = join(website, '..');
const SOURCE = 'https://github.com/beyond10x/takt/blob/main';

function git(...args) {
  try {
    return execFileSync('git', ['-C', repo, ...args], {encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore']}).trim();
  } catch {
    return undefined;
  }
}

// The planning store's front matter is YAML; only the keys read here are parsed, line by line.
function frontMatter(text) {
  const lines = text.split('\n');
  if (lines[0] !== '---') return undefined;
  const out = {relations: []};
  for (const line of lines.slice(1)) {
    if (line === '---') break;
    const top = /^(id|kind|status|title|summary): (.*)$/.exec(line);
    if (top) out[top[1]] = unquote(top[2]);
    const relation = /^- (decomposes|serves|depends_on): (.*)$/.exec(line);
    if (relation) out.relations.push([relation[1], relation[2].trim()]);
  }
  return out;
}

function unquote(value) {
  const v = value.trim();
  if (v.startsWith("'") && v.endsWith("'")) return v.slice(1, -1).replace(/''/g, "'");
  if (v.startsWith('"') && v.endsWith('"')) return JSON.parse(v);
  return v;
}

function readKind(kind) {
  const dir = join(repo, '.engineering', 'planning', kind);
  return readdirSync(dir)
    .filter((f) => f.endsWith('.md'))
    .sort()
    .map((f) => ({file: f, ...frontMatter(readFileSync(join(dir, f), 'utf8'))}));
}

// A flag in a story's title or summary stays on one line. The docs template prints both as plain
// text (StatusTable), where Chrome breaks after a hyphen, the first of the two included ("-" |
// "-allowedTools" once "--" | "allowedTools" is joined); a word joiner, U+2060, after each hyphen
// of the flag forbids those breaks. It is invisible, and what a reader copies differs from the
// title by that character only.
const whole = (text) => text.replace(/(^|[\s(])(--[A-Za-z][\w-]*)/g, (_, before, flag) => before + flag.replace(/-(?=.)/g, '-⁠'));

function status() {
  const epics = Object.fromEntries(
    readKind('epic').map((e) => [e.id, e.title.split(':')[0].trim()]),
  );
  const stories = readKind('story').filter((s) => s.status !== 'archived');
  const items = [];
  const counts = {};
  for (const s of stories) {
    counts[s.status] = (counts[s.status] ?? 0) + 1;
    // Implemented stories are shipped; stories in an open wave (`active`) are decided. Drafts and
    // proposals are counted on the status page and left out of the list.
    const mapped = s.status === 'implemented' ? 'shipped' : s.status === 'active' ? 'decided' : undefined;
    if (!mapped) continue;
    const epic = s.relations.find(([r, t]) => r === 'decomposes' && t.startsWith('epic:'));
    items.push({
      area: epic ? epics[epic[1]] ?? epic[1] : 'Other',
      label: whole(s.title),
      status: mapped,
      ...(s.summary ? {detail: whole(s.summary)} : {}),
      href: `${SOURCE}/.engineering/planning/story/${s.file}`,
    });
  }
  const rank = {shipped: 0, decided: 1};
  items.sort((a, b) => a.area.localeCompare(b.area) || rank[a.status] - rank[b.status] || a.label.localeCompare(b.label));
  const revision = git('rev-parse', '--short', 'HEAD');
  const asOf = git('log', '-1', '--format=%cs');
  const document = {
    format: 'b10x-status/1',
    ...(asOf ? {asOf} : {}),
    source: `.engineering/planning at ${revision ?? 'an unknown revision'}, by website/scripts/generate.mjs`,
    items,
  };
  mkdirSync(join(website, 'data'), {recursive: true});
  writeFileSync(join(website, 'data', 'status.json'), JSON.stringify(document, null, 2) + '\n');
  writeFileSync(
    join(website, 'data', 'story-counts.json'),
    JSON.stringify({revision: revision ?? null, counts, total: stories.length}, null, 2) + '\n',
  );
  console.log(`status: ${items.length} items of ${stories.length} stories (${JSON.stringify(counts)})`);
}

// The charts follow the operating system's colour scheme; on the site they follow its own theme.
function themed(svg) {
  return svg.replace(/@media \(prefers-color-scheme: dark\)\{((?:[^{}]*\{[^{}]*\})*)\}/g, (_, body) =>
    body.replace(/(^|\})([^{}]+)\{/g, (m, close, selector) => `${close}[data-theme='dark'] ${selector.trim()}{`),
  );
}

function charts() {
  const out = {};
  for (const name of ['timeline', 'journey']) {
    out[name] = themed(readFileSync(join(repo, 'docs', 'metrics', `${name}.svg`), 'utf8'));
  }
  mkdirSync(join(website, 'src', 'generated'), {recursive: true});
  const body = Object.entries(out)
    .map(([name, svg]) => `export const ${name} = ${JSON.stringify(svg)};`)
    .join('\n');
  writeFileSync(join(website, 'src', 'generated', 'charts.js'), `// generated by website/scripts/generate.mjs from docs/metrics/\n${body}\n`);
  console.log('charts: timeline.svg, journey.svg');
}

// The CLI reference and the exit codes, from the binary's own help: built with cargo from this
// checkout, or TAKT_BIN when it names a binary.
function takt(args) {
  const bin = process.env.TAKT_BIN;
  const [program, argv] = bin ? [bin, args] : ['cargo', ['run', '--locked', '--quiet', '-p', 'b10x-takt-cli', '--', ...args]];
  return execFileSync(program, argv, {cwd: repo, encoding: 'utf8', stdio: ['ignore', 'pipe', 'inherit']});
}

const help = (args) => takt([...args, '--help']);

// Every command the help lists, in its order, and a command with commands of its own as each of
// them (`scenario run`): a command added to the binary gets its section with no edit here.
function commands(args = []) {
  const text = help(args);
  const at = text.indexOf('\nCommands:\n');
  if (at < 0) return [args];
  const names = [];
  for (const line of text.slice(at + 11).split('\n')) {
    if (!line.startsWith('  ')) break;
    const m = /^  (\S+)\s/.exec(line);
    if (m && m[1] !== 'help') names.push(m[1]);
  }
  return names.flatMap((name) => commands([...args, name]));
}

function exitCodes(text) {
  const at = text.indexOf('\nExit codes:\n');
  if (at < 0) return [];
  const rows = [];
  for (const line of text.slice(at + 13).split('\n')) {
    const m = /^  (\d+)  (.*)$/.exec(line);
    if (m) rows.push([m[1], m[2]]);
    else if (/^\s{4,}\S/.test(line) && rows.length) rows[rows.length - 1][1] += ' ' + line.trim();
  }
  return rows;
}

// A table cell from help text. `|` is escaped everywhere, since the table splits its cells before
// code spans are read. `<` and `>` are escaped outside code spans only: Markdown decodes no entity
// inside one, which showed `&lt;case&gt;` on the page. A flag outside a code span becomes one, and
// a table's code does not wrap, so `--baseline-only` never breaks after its `--`.
const cell = (s) =>
  s
    .split(/(`[^`]*`)/)
    .map((part, i) =>
      i % 2
        ? part
        : part
            .replace(/</g, '&lt;')
            .replace(/>/g, '&gt;')
            .replace(/(^|[\s(])(--[a-z][a-z0-9-]*)/g, '$1`$2`'),
    )
    .join('')
    .replace(/\|/g, '\\|');

// Whose help the pages show, in words. `takt --version` prints Cargo.toml's version and the commit
// it was built from, and between releases `main` keeps the last release's number: "takt 0.6.0"
// alone would name a release whose help this is not. The site is built from `main`
// (.github/workflows/pages.yml, a shallow checkout without tags, so how far it is past the release
// is known only where the tags are).
function origin(version) {
  const m = /^takt (\S+) \(([0-9a-f]{7,40})(-dirty)?\)$/.exec(version);
  if (!m) return `of \`${version}\``;
  const [, number, sha, dirty] = m;
  const commit = `[\`${sha.slice(0, 8)}\`](https://github.com/beyond10x/takt/commit/${sha})`;
  const changes = dirty ? ', with uncommitted changes' : '';
  if (!dirty && git('describe', '--tags', '--exact-match', sha) === number) return `of release ${number} (commit ${commit})`;
  const ref = process.env.GITHUB_REF ?? '';
  const branch = process.env.GITHUB_HEAD_REF || (ref.startsWith('refs/heads/') ? ref.slice(11) : git('symbolic-ref', '--short', '-q', 'HEAD'));
  const built = branch ? `\`${branch}\` at commit ${commit}` : `commit ${commit}`;
  const after = /^(.+)-(\d+)-g[0-9a-f]+$/.exec(git('describe', '--tags', sha) ?? '');
  const where = after
    ? `${after[2]} commit${after[2] === '1' ? '' : 's'} after release ${after[1]}, whose version number it still prints`
    : `which can be ahead of release ${number}, whose version number it prints`;
  return (
    `of takt built from ${built}${changes}, ${where}. What it has beyond the release is under ` +
    `Unreleased in the [changelog](${SOURCE}/CHANGELOG.md); the release's own \`takt --help\` does not list it`
  );
}

function cli() {
  const version = takt(['--version']).trim();
  const of = origin(version);
  const head = (title, description) =>
    `---\ntitle: ${title}\ndescription: ${description}\ncustom_edit_url: null\n---\n\n<!-- generated by website/scripts/generate.mjs from takt --help; do not edit -->\n\n# ${title}\n\n`;
  let page = head('CLI', 'Every takt command and its options, from the binary\'s own help.');
  page += `Generated from \`takt <command> --help\` ${of}.\n\n`;
  page += '```text\n' + help([]).trim() + '\n```\n';
  let codes = head('Exit codes', 'What each takt command\'s exit code means, from the binary\'s own help.');
  codes += `Generated from \`takt <command> --help\` ${of}. A run's outcome lives in files and the exit code, never only in a process, so a script or an orchestrator can drive takt from the code alone.\n`;
  const all = commands();
  for (const args of all) {
    const name = `takt ${args.join(' ')}`;
    const text = help(args).trim();
    page += `\n## ${name}\n\n\`\`\`text\n${text}\n\`\`\`\n`;
    const rows = exitCodes(text);
    if (rows.length) {
      codes += `\n## ${name}\n\n| code | meaning |\n|---|---|\n` + rows.map(([c, m]) => `| ${c} | ${cell(m)} |`).join('\n') + '\n';
    }
  }
  const dir = join(website, 'docs', 'reference');
  writeFileSync(join(dir, 'cli.md'), page);
  writeFileSync(join(dir, 'exit-codes.md'), codes);
  console.log(`cli: ${all.length} commands (${all.map((a) => a.join(' ')).join(', ')}) from ${version}`);
}

function viewer() {
  const target = join(website, 'static', 'protocols');
  rmSync(target, {recursive: true, force: true});
  if (process.env.TAKT_SITE_SKIP_VIEWER === '1') {
    console.log('viewer: skipped');
    return;
  }
  const app = join(repo, 'docs', 'site', 'protocols');
  execFileSync('npm', ['ci', '--ignore-scripts', '--no-audit', '--no-fund'], {cwd: app, stdio: 'inherit'});
  execFileSync('npm', ['run', 'build'], {cwd: app, stdio: 'inherit'});
  cpSync(join(app, 'dist'), target, {recursive: true});
  console.log('viewer: static/protocols/');
}

status();
charts();
cli();
viewer();
