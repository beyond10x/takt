// Where code and a table header may break: between words only. A browser breaks a line
// after a hyphen or a slash, which split `--rejudge` after its `--` and `held-out` after its `-`.
// A word holding such a character is kept whole (custom.css, `.takt-word`); one longer than
// LONG_WORD may still wrap when it is wider than its line (`.takt-word--long`).
// Used by scripts/rehype-layout.mjs for table headers, src/theme/CodeInline.tsx for inline code
// and src/theme/CodeBlock/Line/Token.tsx for code blocks.

export const LONG_WORD = 24;

// The text as its parts in order: {space} between words, {word} for a word that cannot break
// inside itself (letters, digits, `_` and `.`), {word, keep} for one to keep whole.
export function words(value) {
  return value
    .split(/(\s+)/)
    .filter((part) => part !== '')
    .map((part) => {
      if (/^\s+$/.test(part)) return {text: part, space: true};
      if (!/[^\w.]/.test(part)) return {text: part};
      return {text: part, keep: part.length > LONG_WORD ? 'long' : 'short'};
    });
}

export const keepClass = (keep) => (keep === 'long' ? 'takt-word takt-word--long' : 'takt-word');

// A line of a two-column list, as a command's help prints one: leading spaces, a name of words
// joined by single spaces (`-h, --help`), two or more spaces, then its description.
const LIST = /^( +)(\S+(?: \S+)*?)( {2,})\S/;

// The column a list line's description starts at, read from the line's own leading spaces, name
// and gap; undefined for any other line. Used by src/theme/CodeBlock/Line/index.tsx.
export function listColumn(line) {
  const list = LIST.exec(line);
  return list ? list[0].length - 1 : undefined;
}
