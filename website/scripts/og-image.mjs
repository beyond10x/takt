// Draws the site's link preview, website/static/img/og.png: 1200 x 630 px, the size og:image and
// a summary_large_image card take (docusaurus.config.ts names it as themeConfig.image). It is the
// site's own mark (static/img/mark.svg), its name, eyebrow and promise (product.json) on the
// docs-system's dark ground, set in the docs-system's self-hosted Inter, in headless Chrome.
// Nothing is fetched: the colours and the font come from the pinned @beyond10x/docs-system.
//
//   node website/scripts/og-image.mjs
//
// Run it again when the mark, the name or the promise changes, and commit the PNG. Chrome is
// CHROME_PATH, else the first of google-chrome-stable, google-chrome, chromium, chromium-browser
// on PATH. Exit 0: written; 2: it could not run.
import {execFileSync} from 'node:child_process';
import {readFileSync} from 'node:fs';
import {dirname, join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {chromium} from 'playwright-core';

const website = join(dirname(fileURLToPath(import.meta.url)), '..');
const system = join(website, 'node_modules', '@beyond10x', 'docs-system', 'styles');
const out = join(website, 'static', 'img', 'og.png');
const WIDTH = 1200;
const HEIGHT = 630;

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

const escape = (text) => text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');

const {product} = JSON.parse(readFileSync(join(website, 'product.json'), 'utf8'));
const mark = readFileSync(join(website, 'static', 'img', 'mark.svg'), 'utf8');
const theme = readFileSync(join(system, 'product-theme.css'), 'utf8');
const inter = readFileSync(join(system, 'static', 'b10x-fonts', 'inter-variable-latin.woff2')).toString('base64');
const lines = product.promise.slice(0, -1);
const emphasis = product.promise.at(-1);

const page = `<!doctype html>
<html data-theme="dark">
<head>
<meta charset="utf-8">
<style>
@font-face { font-family: Inter; src: url(data:font/woff2;base64,${inter}) format('woff2'); font-weight: 100 900; }
${theme}
html, body { margin: 0; }
body {
  box-sizing: border-box;
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  inline-size: ${WIDTH}px;
  block-size: ${HEIGHT}px;
  padding: 72px 80px 64px;
  background:
    radial-gradient(circle at 92% 8%, color-mix(in srgb, var(--b10x-color-accent) 16%, transparent), transparent 42%),
    var(--b10x-color-canvas);
  color: var(--b10x-color-heading);
  font-family: Inter, sans-serif;
  font-feature-settings: 'cv11', 'ss01';
  -webkit-font-smoothing: antialiased;
}
.brand { display: flex; align-items: center; gap: 28px; }
.brand svg { inline-size: 96px; block-size: 96px; flex: none; }
.name { font-size: 84px; font-weight: 750; letter-spacing: -0.04em; line-height: 1; }
.family { align-self: flex-end; margin-bottom: 10px; color: var(--b10x-color-muted); font-size: 30px; font-weight: 400; }
h1 { margin: 0; font-size: 58px; font-weight: 620; line-height: 1.1; letter-spacing: -0.045em; }
h1 em { display: block; color: var(--b10x-color-accent); font-style: normal; }
.foot { display: flex; justify-content: space-between; align-items: baseline; padding-top: 26px; border-top: 1px solid var(--b10x-color-hairline); color: var(--b10x-color-muted); font-size: 24px; }
.kicker { color: var(--b10x-color-accent); font-weight: 650; letter-spacing: 0.14em; text-transform: uppercase; font-size: 20px; }
</style>
</head>
<body>
<div class="brand">${mark}<span class="name">${escape(product.name)}</span><span class="family">/ beyond10x</span></div>
<h1>${lines.map((line) => `${escape(line)}<br>`).join('')}<em>${escape(emphasis)}</em></h1>
<div class="foot"><span class="kicker">${escape(product.eyebrow)}</span><span>beyond10x.github.io/${escape(product.name)}</span></div>
</body>
</html>`;

const executablePath = chrome();
if (!executablePath) {
  console.error('og-image: no Chrome found; set CHROME_PATH');
  process.exit(2);
}
const browser = await chromium.launch({executablePath, headless: true});
try {
  const context = await browser.newContext({viewport: {width: WIDTH, height: HEIGHT}, deviceScaleFactor: 1});
  const tab = await context.newPage();
  await tab.setContent(page, {waitUntil: 'load'});
  await tab.evaluate(() => document.fonts.ready);
  const overflow = await tab.evaluate(() => document.documentElement.scrollWidth > innerWidth || document.documentElement.scrollHeight > innerHeight);
  if (overflow) throw new Error('og-image: the preview does not fit 1200 x 630');
  await tab.screenshot({path: out, type: 'png'});
  console.log(`og-image: wrote ${out.slice(website.length + 1)} (${WIDTH} x ${HEIGHT})`);
} finally {
  await browser.close();
}
