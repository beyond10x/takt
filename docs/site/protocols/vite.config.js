import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

// The viewer is a page of the documentation site: website/scripts/generate.mjs copies the build to
// <baseUrl>protocols/. What it shares with the site's own pages is read from the site's config at
// build time, so the two cannot drift: the address (the canonical link and the asset base), the
// navbar (title, mark and links), and the localStorage key under which the site keeps the
// reader's colour mode choice.
const CONFIG = fileURLToPath(new URL("../../../website/docusaurus.config.ts", import.meta.url));
const PAGE = "protocols/";

function site() {
  const text = readFileSync(CONFIG, "utf8");
  const one = (pattern, what, from = text) => {
    const m = from.match(pattern);
    if (!m) throw new Error(`${CONFIG}: no ${what} found; the protocol viewer reads it (docs/site/protocols/vite.config.js)`);
    return m[1];
  };
  const url = one(/^\s*url: '([^']+)'/m, "url");
  const baseUrl = one(/^\s*baseUrl: '([^']+)'/m, "baseUrl");
  const navbar = one(/^\s*navbar: \{\n([\s\S]*?)^\s*\},\n\s*footer:/m, "themeConfig.navbar");
  const title = one(/^\s*title: '([^']+)'/m, "navbar title", navbar);
  const mark = one(/withProductSite\([^)]*\bmark: '([^']+)'/, "product-site mark");
  const nav = [...navbar.matchAll(/\{(to|href): '([^']+)', label: '([^']+)', position: '(left|right)'/g)].map(
    ([, kind, target, label, position]) => {
      const path = target.replace(/^pathname:\/\//, "");
      const href = kind === "to" ? baseUrl.replace(/\/$/, "") + (path === "/" ? "/" : path.replace(/\/$/, "")) : path;
      // Docusaurus draws every `href` item, a pathname:// one too, as an external link: a new tab
      // and the external-link icon. The viewer draws them the same, so its navbar matches.
      return { label, href, position, external: kind === "href", current: href === baseUrl + PAGE };
    },
  );
  if (!nav.length) throw new Error(`${CONFIG}: no navbar items found; the protocol viewer reads them`);
  // The social preview image the site's pages name (themeConfig.image and its metadata).
  const image = new URL(baseUrl + one(/^\s*image: '([^']+)'/m, "themeConfig.image"), url).href;
  const meta = Object.fromEntries(
    [...text.matchAll(/\{(property|name): '((?:og|twitter):image:[a-z]+)', content: '([^']+)'\}/g)].map(([, attr, name, content]) => [name, { attr, content }]),
  );
  for (const name of ["og:image:width", "og:image:height", "og:image:alt", "twitter:image:alt"]) {
    if (!meta[name]) throw new Error(`${CONFIG}: no ${name} in themeConfig.metadata; the protocol viewer reads it`);
  }
  // Docusaurus stores the choice under "theme", namespaced by the first three hex digits of the
  // MD5 of the site's URL when site storage is namespaced, which future.v4 turns on
  // (@docusaurus/core lib/server/storage.js).
  if (/\bstorage\s*:|siteStorageNamespacing/.test(text)) {
    throw new Error(`${CONFIG}: an explicit storage config is not read here; derive the theme key from it in docs/site/protocols/vite.config.js`);
  }
  const home = new URL(baseUrl, url).href;
  const namespaced = /\bv4:\s*true\b/.test(text);
  const themeKey = "theme" + (namespaced ? "-" + createHash("md5").update(home).digest("hex").slice(0, 3) : "");
  return { url: home, baseUrl, title, mark, nav, themeKey, image, meta };
}

const SITE = site();

// Runs before the first paint, as the site's own pages do (Docusaurus's inline theme script): the
// stored choice, else the system's scheme, on <html> as data-theme and data-theme-choice.
const THEME = `(function(){var k=${JSON.stringify(SITE.themeKey)},c=null;` +
  `try{c=new URLSearchParams(location.search).get("docusaurus-theme")||localStorage.getItem(k)}catch(e){}` +
  `if(c!=="light"&&c!=="dark")c=null;var r=document.documentElement;` +
  `r.setAttribute("data-theme",c||(matchMedia("(prefers-color-scheme: dark)").matches?"dark":"light"));` +
  `r.setAttribute("data-theme-choice",c||"system")})()`;

// The site's own fonts (the docs-system product layer serves them under <baseUrl>b10x-fonts/).
const font = (file) => `${SITE.baseUrl}b10x-fonts/${file}`;
const FACES = [
  ["Inter", "inter-variable-latin.woff2", "100 900"],
  ["Fira Code", "fira-code-regular-latin.woff2", "400"],
  ["Fira Code", "fira-code-semibold-latin.woff2", "600"],
].map(([family, file, weight]) => `@font-face{font-family:"${family}";src:url("${font(file)}") format("woff2");font-weight:${weight};font-style:normal;font-display:swap}`).join("");

// Added after Vite has processed index.html, so the site's paths are written as they are.
function head() {
  return {
    name: "takt-site-head",
    transformIndexHtml: {
      order: "post",
      handler: () => [
        { tag: "script", children: THEME, injectTo: "body-prepend" },
        { tag: "link", attrs: { rel: "icon", href: `${SITE.baseUrl}img/mark.svg` }, injectTo: "head" },
        { tag: "link", attrs: { rel: "canonical", href: SITE.url + PAGE }, injectTo: "head" },
        { tag: "meta", attrs: { property: "og:url", content: SITE.url + PAGE }, injectTo: "head" },
        // The preview image as the site's pages name it (Docusaurus writes og:image, twitter:image
        // and twitter:card from themeConfig.image, and the rest from themeConfig.metadata).
        { tag: "meta", attrs: { property: "og:image", content: SITE.image }, injectTo: "head" },
        ...Object.entries(SITE.meta).map(([name, { attr, content }]) => ({ tag: "meta", attrs: { [attr]: name, content }, injectTo: "head" })),
        { tag: "meta", attrs: { name: "twitter:card", content: "summary_large_image" }, injectTo: "head" },
        { tag: "meta", attrs: { name: "twitter:image", content: SITE.image }, injectTo: "head" },
        ...["inter-variable-latin.woff2", "fira-code-regular-latin.woff2"].map((file) => ({
          tag: "link", attrs: { rel: "preload", href: font(file), as: "font", type: "font/woff2", crossorigin: "anonymous" }, injectTo: "head",
        })),
        { tag: "style", children: FACES, injectTo: "head" },
      ],
    },
  };
}

export default defineConfig({
  base: SITE.baseUrl + PAGE,
  plugins: [vue(), head()],
  define: { __SITE__: JSON.stringify(SITE) },
  build: { outDir: "dist", emptyOutDir: true },
});
