import type {CSSProperties, ReactNode} from 'react';
import type {Props} from '@theme/CodeBlock/Line';
import Line from '@theme-original/CodeBlock/Line';
import {LONG_WORD, listColumn} from '@site/src/words.mjs';

// A wrapped line of a two-column list goes on under its description: its hang is the
// description's column (src/words.mjs), which custom.css (`.takt-list`) caps so that at least
// LONG_WORD columns are left beside it, min(column, 100cqi - both paddings - LONG_WORD ch).
// LONG_WORD (src/words.mjs) reaches the CSS as `--takt-long-word`, set here. Every other line
// keeps the hang its block gives it. The line's text is unchanged.
export default function CodeBlockLine(props: Props): ReactNode {
  const column = listColumn(props.line.map((token) => token.content).join(''));
  if (column === undefined) return <Line {...props} />;
  const hang = {
    '--takt-list-hang': `${column}ch`,
    '--takt-long-word': `${LONG_WORD}ch`,
  } as CSSProperties;
  return (
    <Line
      {...props}
      classNames={[...(props.classNames ?? []), 'takt-list']}
      getLineProps={(input: Parameters<Props['getLineProps']>[0]) => {
        const out = props.getLineProps(input);
        return {...out, style: {...out.style, ...hang}};
      }}
    />
  );
}
