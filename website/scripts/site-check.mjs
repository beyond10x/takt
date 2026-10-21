// Screenshots every page of the built site at 1440 and 390 px wide with headless Chrome, and fails
// when a page scrolls horizontally, when any HTML text, a ::before's or ::after's included, renders
// below 12 px or a chart's SVG text below 11 px, when two text boxes of one chart intersect, when
// a page logs a console error or a request of it fails, when a navbar label wraps or the navbar's
// wordmark is cut, or when a page does not follow the reader's colour mode choice stored by the
// site. The protocol viewer is checked at 1024, 1000 and 768 px too, and a path the site does not
// have must answer 404 with the not-found page's title and noindex.
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
// The viewer's navbar wrapped and its wordmark was cut from 997 to 1009 px, and its tables made it
// scroll sideways from 641 to 806 px.
const VIEWER = '/takt/protocols/';
const EXTRA = [1024, 1000, 768];
const MISSING = '/takt/no-such-page';
const NOT_FOUND_TITLE = 'Page not found | takt';
// A chart's text (SVG, scaled with its chart) and every other text: the floor custom.css raises
// the docs-system's small labels to.
const MIN_PX = 11;
const MIN_TEXT_PX = 12;

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

// A path with no file gets the build's 404.html and status 404, as GitHub Pages answers it.
function serve() {
  const server = createServer((req, res) => {
    const f = file(req.url ?? '/');
    if (!f) {
      const notFound = join(build, '404.html');
      if (existsSync(notFound) && (req.headers.accept ?? '').includes('text/html')) {
        res.writeHead(404, {'content-type': 'text/html'});
        res.end(readFileSync(notFound));
        return;
      }
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
function inspect(floors) {
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
  const note = (el, px, floor, text, part = '') => {
    if (px >= floor) return;
    const key = `${floor} px: ${selector(el)}${part} at ${px.toFixed(1)} px`;
    const entry = small.get(key) ?? {n: 0, text};
    entry.n += 1;
    small.set(key, entry);
  };
  for (let node = walker.nextNode(); node; node = walker.nextNode()) {
    const text = node.textContent.trim();
    if (!text) continue;
    const el = node.parentElement;
    if (!el || seen.has(el) || ['TITLE', 'STYLE', 'SCRIPT', 'DESC'].includes(el.tagName.toUpperCase())) continue;
    seen.add(el);
    if (!visible(el)) continue;
    const chart = el instanceof SVGElement || Boolean(el.closest('foreignObject'));
    note(el, parseFloat(getComputedStyle(el).fontSize) * scaleOf(el), chart ? floors.chart : floors.text, text);
  }
  // Text a stylesheet draws, a ::before's or ::after's content: a code block's language, a card's
  // counter.
  for (const el of document.body.querySelectorAll('*')) {
    if (el instanceof SVGElement || !visible(el)) continue;
    for (const part of ['::before', '::after']) {
      const style = getComputedStyle(el, part);
      if (style.display === 'none' || style.visibility === 'hidden') continue;
      const drawn = (style.content === 'none' || style.content === 'normal' ? '' : style.content).replace(/url\([^)]*\)/g, '').replace(/["']/g, '').trim();
      if (drawn) note(el, parseFloat(style.fontSize), floors.text, drawn, part);
    }
  }
  for (const [key, {n, text}] of small) failures.push(`text below ${key}, ${n} element(s), e.g. "${text.slice(0, 40)}"`);
  // The navbar's labels stay on one line and its wordmark whole (custom.css; the viewer's
  // style.css).
  for (const link of document.querySelectorAll('.navbar__items .navbar__link')) {
    if (!visible(link)) continue;
    const tops = new Set();
    for (const node of link.childNodes) {
      if (node.nodeType !== Node.TEXT_NODE || !node.textContent.trim()) continue;
      const range = document.createRange();
      range.selectNodeContents(node);
      for (const r of range.getClientRects()) if (r.width > 0) tops.add(Math.round(r.top));
    }
    if (tops.size > 1) failures.push(`navbar link "${link.textContent.trim()}" wraps onto ${tops.size} lines`);
  }
  const title = document.querySelector('.navbar__title');
  if (title && visible(title) && title.scrollWidth > title.clientWidth + 1) {
    failures.push(`navbar title cut: scrollWidth ${title.scrollWidth} > clientWidth ${title.clientWidth}`);
  }
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
  let checked = 0;
  try {
    // Every page at WIDTHS; the protocol viewer, which is not a Docusaurus page and has its own
    // stylesheet, also where its navbar and its tables are tightest (EXTRA).
    const runs = WIDTHS.map((width) => [width, list]);
    if (list.includes(VIEWER)) runs.push(...EXTRA.map((width) => [width, [VIEWER]]));
    for (const [width, paths] of runs) {
      const dir = join(out, String(width));
      mkdirSync(dir, {recursive: true});
      const context = await browser.newContext({viewport: {width, height: 900}, colorScheme: 'light'});
      const page = await context.newPage();
      const errors = [];
      page.on('pageerror', (e) => errors.push(`page error: ${String(e)}`));
      page.on('console', (m) => {
        if (m.type() === 'error') errors.push(`console error: ${m.text()}`);
      });
      page.on('requestfailed', (r) => errors.push(`request failed: ${r.url()}`));
      page.on('response', (r) => {
        if (r.status() >= 400) errors.push(`answered ${r.status()}: ${r.url()}`);
      });
      for (const path of paths) {
        checked += 1;
        errors.length = 0;
        const response = await page.goto(origin + path, {waitUntil: 'networkidle'});
        await page.evaluate(() => document.fonts.ready);
        await page.waitForTimeout(300);
        const failures = response && response.ok() ? await page.evaluate(inspect, {chart: MIN_PX, text: MIN_TEXT_PX}) : [`answered ${response ? response.status() : 'nothing'}`];
        for (const e of errors) failures.push(e.slice(0, 160));
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
    // A path the site does not have answers 404 with the not-found page: its own title, and
    // noindex so search engines leave it out.
    {
      const context = await browser.newContext({viewport: {width: WIDTHS[0], height: 900}, colorScheme: 'light'});
      const page = await context.newPage();
      const response = await page.goto(origin + MISSING, {waitUntil: 'networkidle'});
      const found = await page.evaluate(() => ({
        title: document.title,
        robots: document.querySelector('meta[name="robots"]')?.getAttribute('content') ?? null,
      }));
      const status = response ? response.status() : null;
      const failures = [];
      if (status !== 404) failures.push(`answered ${status ?? 'nothing'}, not 404`);
      if (found.title !== NOT_FOUND_TITLE) failures.push(`title "${found.title}", not "${NOT_FOUND_TITLE}"`);
      if (!/\bnoindex\b/.test(found.robots ?? '')) failures.push(`robots ${found.robots ?? 'missing'}, not noindex`);
      checked += 1;
      if (failures.length) {
        failed += 1;
        console.log(`FAIL ${WIDTHS[0]} ${MISSING}`);
        for (const f of failures) console.log(`  - ${f}`);
      } else {
        console.log(`ok   ${WIDTHS[0]} ${MISSING} (404, "${found.title}", ${found.robots})`);
      }
      await context.close();
    }
    // The reader's choice on the site, stored under the site's key, wins over the system's scheme
    // on every page, the protocol viewer's too (it is not a Docusaurus page and reads the key
    // itself, docs/site/protocols/src/theme.js).
    const key = readFileSync(join(build, 'index.html'), 'utf8').match(/localStorage\.getItem\("(theme[^"]*)"\)/)?.[1];
    if (!key) {
      failed += 1;
      console.log('FAIL colour mode: no theme key in the inline script of index.html');
    } else {
      const context = await browser.newContext({viewport: {width: WIDTHS[0], height: 900}, colorScheme: 'light'});
      await context.addInitScript(([k]) => localStorage.setItem(k, 'dark'), [key]);
      const page = await context.newPage();
      for (const path of list) {
        await page.goto(origin + path, {waitUntil: 'load'});
        const theme = await page.evaluate(() => document.documentElement.getAttribute('data-theme'));
        if (theme !== 'dark') {
          failed += 1;
          console.log(`FAIL colour mode ${path}`);
          console.log(`  - the site's choice is dark (${key}) and the system's light; the page is ${theme ?? 'unthemed'}`);
        }
      }
      await context.close();
    }
  } finally {
    await browser.close();
    server.close();
  }
  console.log(`site-check: ${list.length} pages x ${WIDTHS.length} widths, the viewer at ${EXTRA.length} more, ${MISSING}; ${checked} checks, ${failed} failed; screenshots in ${out}`);
  return failed ? 1 : 0;
}

process.exitCode = await main().catch((e) => {
  console.error(`site-check: ${e.stack ?? e}`);
  return 2;
});
