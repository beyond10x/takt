// A rehype pass over every docs page (docusaurus.config.ts). It runs before the docs-system's chip
// pass, which withProductSite appends after the site's own plugins, and adds what the template's
// layout lacks:
//
//   - every table sits in a frame of its own, `div.takt-scroll`, that scrolls sideways when the
//     table is wider than the column (custom.css shades the edge that has more). The template
//     scrolls a table only under 997 px, so above that a wide one ran under the contents rail.
//   - a table header, and a cell that is one word, break only between words (src/words.mjs), so
//     `held-out` and `metaharness-1` stay whole. Code gets the same at render time, in
//     src/theme/CodeInline.tsx and src/theme/CodeBlock/Line/Token.tsx.
//   - a word stays what the Markdown wrote. The chip pass turns inline code or a whole table cell
//     that reads `action`, `evidence`, `claim`, `outcome` or `obligation` into a protocol-kind
//     chip, so a field named `action` read as a chip among plain code. Such code is written as a
//     JSX `<code>` and such a cell's text as a JSX `<span>`, which the chip pass leaves alone and
//     which render as the Markdown would.
import {keepClass, words} from '../src/words.mjs';

const KIND_WORDS = new Set(['action', 'evidence', 'claim', 'outcome', 'obligation']);

const jsx = (name, value) => ({type: 'mdxJsxTextElement', name, attributes: [], children: [{type: 'text', value}]});

const wordsOf = (value) =>
  words(value).map(({text, keep}) =>
    keep
      ? {type: 'element', tagName: 'span', properties: {className: keepClass(keep).split(' ')}, children: [{type: 'text', value: text}]}
      : {type: 'text', value: text},
  );

const kindWord = (node) => node.type === 'text' && KIND_WORDS.has(node.value.trim());

function walk(node) {
  if (!Array.isArray(node.children)) return;
  node.children = node.children.map((child) => {
    if (child.type !== 'element' || child.tagName === 'pre') return child;
    if (child.tagName === 'code') {
      const [only, ...rest] = child.children;
      return only && !rest.length && kindWord(only) ? jsx('code', only.value) : child;
    }
    if (child.tagName === 'td') {
      const meaningful = child.children.filter((c) => !(c.type === 'text' && !c.value.trim()));
      if (meaningful.length === 1 && kindWord(meaningful[0])) {
        child.children = [jsx('span', meaningful[0].value.trim())];
        return child;
      }
      if (meaningful.length === 1 && meaningful[0].type === 'text' && !/\s/.test(meaningful[0].value.trim())) {
        child.children = wordsOf(meaningful[0].value.trim());
        return child;
      }
    }
    if (child.tagName === 'th') {
      child.children = child.children.flatMap((c) => (c.type === 'text' ? wordsOf(c.value) : [c]));
    }
    walk(child);
    if (child.tagName === 'table') {
      return {type: 'element', tagName: 'div', properties: {className: ['takt-scroll']}, children: [child]};
    }
    return child;
  });
}

export default function rehypeLayout() {
  return (tree) => walk(tree);
}
