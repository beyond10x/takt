import type {CSSProperties, ReactNode} from 'react';

// The smallest text the site check accepts (website/scripts/site-check.mjs, MIN_PX).
const FLOOR_PX = 11;

// The chart's drawn width and its smallest font size, read from its source.
function measure(svg: string): {width: number; font: number} | undefined {
  const root = /<svg\b[^>]*>/.exec(svg)?.[0] ?? '';
  const width = Number(/\bwidth="([\d.]+)"/.exec(root)?.[1]);
  const sizes = [...svg.matchAll(/font-size(?:="|:\s*)([\d.]+)/g)].map((m) => Number(m[1]));
  if (!width || !sizes.length) return undefined;
  return {width, font: Math.min(...sizes)};
}

// A chart from docs/metrics/, inlined so its text is real text. It shrinks to its column down to
// the width at which its smallest text is still FLOOR_PX, never past its drawn size; on a column
// narrower than that it keeps that width and scrolls sideways inside its frame, which shades the
// edge that has more (custom.css).
export default function Chart({svg, caption}: {svg: string; caption?: ReactNode}): ReactNode {
  const size = measure(svg);
  // The frame's overflow is custom.css's, so its print rule can show the whole chart.
  const style = {
    maxWidth: '100%',
    ...(size && {
      '--takt-chart-width': `${size.width}px`,
      '--takt-chart-min': `${Math.ceil((size.width * FLOOR_PX) / size.font) + 1}px`,
    }),
  } as CSSProperties;
  return (
    <figure style={{margin: '1.5rem 0'}}>
      <div className="takt-chart" tabIndex={0} style={style} dangerouslySetInnerHTML={{__html: svg}} />
      {caption && <figcaption style={{fontSize: '0.9rem', marginTop: '0.6rem'}}>{caption}</figcaption>}
    </figure>
  );
}
