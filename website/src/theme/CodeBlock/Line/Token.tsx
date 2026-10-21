import type {ReactNode} from 'react';
import type {Props} from '@theme/CodeBlock/Line/Token';
import {keepClass, words} from '@site/src/words.mjs';

type Part = {text: string; space?: true; keep?: 'short' | 'long'};

// A code block wraps (custom.css), and breaks only between words (src/words.mjs), so a wrapped
// help line never splits `--baseline-only` after a hyphen. Docusaurus's own token is a bare span.
export default function CodeBlockLineToken({line, token, children, ...props}: Props): ReactNode {
  if (typeof children !== 'string') return <span {...props}>{children}</span>;
  return (
    <span {...props}>
      {(words(children) as Part[]).map(({text, keep}, i) =>
        keep ? (
          <span key={i} className={keepClass(keep)}>
            {text}
          </span>
        ) : (
          text
        ),
      )}
    </span>
  );
}
