import {Children, type ComponentProps, type ReactNode} from 'react';
import {keepClass, words} from '@site/src/words.mjs';

type Part = {text: string; space?: true; keep?: 'short' | 'long'};

// Inline code that breaks only between its words (src/words.mjs): `--rejudge` no longer splits
// after its `--`. Docusaurus renders Markdown's inline code through this component; its own is a
// bare <code>.
export default function CodeInline({children, ...props}: ComponentProps<'code'>): ReactNode {
  const parts = Children.toArray(children);
  if (!parts.every((part) => typeof part === 'string')) return <code {...props}>{children}</code>;
  return (
    <code {...props}>
      {(words(parts.join('')) as Part[]).map(({text, keep}, i) =>
        keep ? (
          <span key={i} className={keepClass(keep)}>
            {text}
          </span>
        ) : (
          text
        ),
      )}
    </code>
  );
}
