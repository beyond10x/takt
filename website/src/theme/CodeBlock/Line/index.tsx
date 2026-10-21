import type {CSSProperties, ReactNode} from 'react';
import type {Props} from '@theme/CodeBlock/Line';
import Line from '@theme-original/CodeBlock/Line';
import {listColumn} from '@site/src/words.mjs';

// A wrapped line of a two-column list goes on under its description (custom.css, `.takt-list`,
// which caps the hang at 40 % of the block): its hang is the description's column
// (src/words.mjs). Every other line keeps the hang its block gives it. The line's text is
// unchanged.
export default function CodeBlockLine(props: Props): ReactNode {
  const column = listColumn(props.line.map((token) => token.content).join(''));
  if (column === undefined) return <Line {...props} />;
  const hang = {'--takt-list-hang': `${column}ch`} as CSSProperties;
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
