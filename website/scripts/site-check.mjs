// Screenshots every page of the built site at 1440 and 390 px wide with headless Chrome, and fails
// when a page scrolls horizontally, when any HTML or SVG text renders below 11 px, or when two
// text boxes of one chart intersect.
//
//   node website/scripts/site-check.mjs [--out <dir>]
//
// Reads website/build (run `npm --prefix website run build` first) and serves it on a local port
// under /takt/. The pages are the build's sitemap plus /takt/protocols/. Screenshots go to
// website/.site-check/<width>/ unless --out names another directory. Chrome is CHROME_PATH, else
// the first of google-chrome-stable, google-chrome, chromium, chromium-browser on PATH.
// Exit 0: every page passed; 1: a page failed (each failure printed); 2: the check could not run.
import {execFileSync} from 'node:child_process';
import {existsSync, mkdirSync, readFileSync, statSync} from 'node:fs';
import {createServer} from 'node:http';
import {dirname, extname, join, normalize, resolve} from 'node:path';
import {fileURLToPath} from 'node:url';
import {chromium} from 'playwright-core';

const website = join(dirname(fileURLToPath(import.meta.url)), '..');
const build = join(website, 'build');
const BASE = '/takt/';
const WIDTHS = [1440, 390];
const MIN_PX = 11;

const outArg = process.argv.indexOf('--out');
const out = resolve(outArg > 0 ? process.argv[outArg + 1] : join(website, '.site-check'));

function chrome() {
  if (process.env.CHROME_PATH) return process.env.CHROME_PATH;
  for (const name of ['google-chrome-stable', 'google-chrome', 'chromium', 'chromium-browser']) {
    try {
      return execFileSync('which', [name], {encoding: 'utf8'}).trim();
    } catch {
      // next
    }
  }
  return undefined;
}

const TYPES = {'.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.svg': 'image/svg+xml', '.json': 'application/json', '.png': 'image/png', '.woff2': 'font/woff2', '.woff': 'font/woff', '.xml': 'application/xml', '.ico': 'image/x-icon'};

function file(urlPath) {
  if (!urlPath.startsWith(BASE)) return undefined;
  const rel = normalize(decodeURIComponent(urlPath.slice(BASE.length).split('?')[0]));
  if (rel.startsWith('..')) return undefined;
  const p = join(build, rel);
  for (const candidate of [p, `${p}.html`, join(p, 'index.html')]) {
    if (existsSync(candidate) && statSync(candidate).isFile()) return candidate;
  }
  return undefined;
}

function serve() {
  const server = createServer((req, res) => {
    const f = file(req.url ?? '/');
    if (!f) {
      res.writeHead(404);
      res.end('not found');
      return;
    }
    res.writeHead(200, {'content-type': TYPES[extname(f)] ?? 'application/octet-stream'});
    res.end(readFileSync(f));
  });
  return new Promise((ok) => server.listen(0, '127.0.0.1', () => ok(server)));
}

function pages() {
  const sitemap = readFileSync(join(build, 'sitemap.xml'), 'utf8');
  const paths = [...sitemap.matchAll(/<loc>https:\/\/beyond10x\.github\.io(\/takt\/[^<]*)<\/loc>/g)].map((m) => m[1]);
  if (existsSync(join(build, 'protocols', 'index.html'))) paths.push('/takt/protocols/');
  return [...new Set(paths)].sort();
}

// Runs in the page. Returns the page's failures as strings.
function inspect(minPx) {
  const failures = [];
  const doc = document.documentElement;
  if (doc.scrollWidth > window.innerWidth) {
    failures.push(`scrolls horizontally: scrollWidth ${doc.scrollWidth} > innerWidth ${window.innerWidth}`);
  }
  const visible = (el) => {
    const rects = el.getClientRects();
    if (!rects.length) return false;
    const r = el.getBoundingClientRect();
    if (r.width <= 1 || r.height <= 1) return false;
    const style = getComputedStyle(el);
    return style.visibility !== 'hidden' && style.display !== 'none' && Number(style.opacity) !== 0;
  };
  const scaleOf = (el) => {
    const host = el instanceof SVGElement ? el : el.closest('foreignObject');
    if (!host || typeof host.getScreenCTM !== 'function') return 1;
    const m = host.getScreenCTM();
    return m ? Math.sqrt(Math.abs(m.a * m.d - m.b * m.c)) : 1;
  };
  const selector = (el) => {
    const cls = typeof el.className === 'string' && el.className.trim() ? '.' + el.className.trim().split(/\s+/).join('.') : '';
    const parent = el.parentElement ? el.parentElement.tagName.toLowerCase() + ' > ' : '';
    return `${parent}${el.tagName.toLowerCase()}${cls}`;
  };
  const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
  const small = new Map();
  const seen = new Set();
  for (let node = walker.nextNode(); node; node = walker.nextNode()) {
    const text = node.textContent.trim();
    if (!text) continue;
    const el = node.parentElement;
    if (!el || seen.has(el) || ['TITLE', 'STYLE', 'SCRIPT', 'DESC'].includes(el.tagName.toUpperCase())) continue;
    seen.add(el);
    if (!visible(el)) continue;
    const px = parseFloat(getComputedStyle(el).fontSize) * scaleOf(el);
    if (px < minPx) {
      const key = `${selector(el)} at ${px.toFixed(1)} px`;
      const entry = small.get(key) ?? {n: 0, text};
      entry.n += 1;
      small.set(key, entry);
    }
  }
  for (const [key, {n, text}] of small) failures.push(`text below ${minPx} px: ${key}, ${n} element(s), e.g. "${text.slice(0, 40)}"`);
  let chart = 0;
  for (const svg of document.querySelectorAll('svg')) {
    if (svg.parentElement && svg.parentElement.closest('svg')) continue;
    chart += 1;
    const boxes = [...svg.querySelectorAll('text, foreignObject')]
      .filter((t) => t.textContent.trim() && !t.closest('title') && visible(t))
      .filter((t) => !(t.tagName.toLowerCase() === 'text' && t.closest('foreignObject')))
      .map((t) => ({t, r: t.getBoundingClientRect()}));
    const hits = [];
    for (let i = 0; i < boxes.length; i++) {
      for (let j = i + 1; j < boxes.length; j++) {
        const a = boxes[i].r;
        const b = boxes[j].r;
        const w = Math.min(a.right, b.right) - Math.max(a.left, b.left);
        const h = Math.min(a.bottom, b.bottom) - Math.max(a.top, b.top);
        if (w > 1 && h > 1) hits.push(`"${boxes[i].t.textContent.trim().slice(0, 30)}" / "${boxes[j].t.textContent.trim().slice(0, 30)}"`);
      }
    }
    if (hits.length) {
      const name = svg.getAttribute('aria-label') || svg.querySelector('title')?.textContent || svg.getAttribute('class') || `svg ${chart}`;
      failures.push(`chart "${String(name).slice(0, 50)}": ${hits.length} intersecting text box pair(s), e.g. ${hits.slice(0, 3).join('; ')}`);
    }
  }
  return failures;
}

async function main() {
  if (!existsSync(join(build, 'sitemap.xml'))) {
    console.error(`site-check: no build at ${build}; run npm --prefix website run build first`);
    return 2;
  }
  const executablePath = chrome();
  if (!executablePath) {
    console.error('site-check: no Chrome found; set CHROME_PATH');
    return 2;
  }
  const server = await serve();
  const origin = `http://127.0.0.1:${server.address().port}`;
  const browser = await chromium.launch({executablePath, headless: true});
  const list = pages();
  let failed = 0;
  try {
    for (const width of WIDTHS) {
      const dir = join(out, String(width));
      mkdirSync(dir, {recursive: true});
      const context = await browser.newContext({viewport: {width, height: 900}, colorScheme: 'light'});
      const page = await context.newPage();
      const errors = [];
      page.on('pageerror', (e) => errors.push(String(e)));
      for (const path of list) {
        errors.length = 0;
        const response = await page.goto(origin + path, {waitUntil: 'networkidle'});
        await page.evaluate(() => document.fonts.ready);
        await page.waitForTimeout(300);
        const failures = response && response.ok() ? await page.evaluate(inspect, MIN_PX) : [`answered ${response ? response.status() : 'nothing'}`];
        for (const e of errors) failures.push(`page error: ${e.slice(0, 120)}`);
        const slug = path.slice(BASE.length).replace(/\/$/, '').replace(/\//g, '__') || 'index';
        await page.screenshot({path: join(dir, `${slug}.png`), fullPage: true, timeout: 180000});
        if (failures.length) {
          failed += 1;
          console.log(`FAIL ${width} ${path}`);
          for (const f of failures) console.log(`  - ${f}`);
        } else {
          console.log(`ok   ${width} ${path}`);
        }
      }
      await context.close();
    }
  } finally {
    await browser.close();
    server.close();
  }
  console.log(`site-check: ${list.length} pages x ${WIDTHS.length} widths, ${failed} failed; screenshots in ${out}`);
  return failed ? 1 : 0;
}

process.exitCode = await main().catch((e) => {
  console.error(`site-check: ${e.stack ?? e}`);
  return 2;
});
