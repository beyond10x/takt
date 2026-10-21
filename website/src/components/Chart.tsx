import type {CSSProperties, ReactNode} from 'react';

// The smallest text the site check accepts (website/scripts/site-check.mjs, MIN_PX).
const FLOOR_PX = 11;

// A chart from docs/metrics/ as website/scripts/generate.mjs writes it: its plot, an SVG of its
// own, and the key its generator wrote beside the plot (takt-chart-key/1). A legend entry's mark
// is SVG drawn in a 14 px box with the chart's classes.
export type ChartParts = {
  class: string;
  title: string;
  intro?: string;
  plot: string;
  legend: {mark: string; text: string}[][];
  notes: {lead?: string; text: string; muted?: boolean}[];
};

// The plot's drawn width and its smallest font size, read from its source.
function measure(svg: string): {width: number; font: number} | undefined {
  const root = /<svg\b[^>]*>/.exec(svg)?.[0] ?? '';
  const width = Number(/\bwidth="([\d.]+)"/.exec(root)?.[1]);
  const sizes = [...svg.matchAll(/font-size(?:="|:\s*)([\d.]+)/g)].map((m) => Number(m[1]));
  if (!width || !sizes.length) return undefined;
  return {width, font: Math.min(...sizes)};
}

// A chart from docs/metrics/. Its title, legend and notes are the page's own text and wrap with
// the column, so they stay in view on a phone; only the plot is a picture, inlined so its text is
// real text. The plot shrinks to its column down to the width at which its smallest text is still
// FLOOR_PX, never past its drawn size; on a column narrower than that it keeps that width and
// scrolls sideways inside its frame, which shades the edge that has more (custom.css).
export default function Chart({chart, caption}: {chart: ChartParts; caption?: ReactNode}): ReactNode {
  const size = measure(chart.plot);
  // The frame's overflow is custom.css's, so its print rule can show the whole chart.
  const style = {
    maxWidth: '100%',
    ...(size && {
      '--takt-chart-width': `${size.width}px`,
      '--takt-chart-min': `${Math.ceil((size.width * FLOOR_PX) / size.font) + 1}px`,
    }),
  } as CSSProperties;
  // generate.mjs labels the plot with this id.
  const heading = `${chart.class}-heading`;
  return (
    <figure className="takt-figure" aria-labelledby={heading}>
      <p className="takt-figure__title" id={heading}>
        {chart.title}
      </p>
      {chart.intro && <p className="takt-figure__intro">{chart.intro}</p>}
      <div className="takt-chart" tabIndex={0} style={style} dangerouslySetInnerHTML={{__html: chart.plot}} />
      {chart.legend.map((group, g) => (
        <ul className="takt-figure__legend" key={g}>
          {group.map(({mark, text}) => (
            <li key={text}>
              <svg
                className={chart.class}
                viewBox="0 0 14 14"
                width="14"
                height="14"
                aria-hidden="true"
                focusable="false"
                dangerouslySetInnerHTML={{__html: mark}}
              />
              <span>{text}</span>
            </li>
          ))}
        </ul>
      ))}
      {chart.notes.map(({lead, text, muted}) => (
        <p className={muted ? 'takt-figure__note takt-figure__note--muted' : 'takt-figure__note'} key={(lead ?? '') + text}>
          {lead && <strong>{lead}</strong>}
          {text}
        </p>
      ))}
      {caption && <figcaption className="takt-figure__caption">{caption}</figcaption>}
    </figure>
  );
}
