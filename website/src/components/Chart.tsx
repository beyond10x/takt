import type {ReactNode} from 'react';

// A chart from docs/metrics/, inlined so its text is real text. It keeps its drawn size (no text
// under 12 px) and scrolls sideways inside its frame on a narrow screen instead of shrinking.
export default function Chart({svg, caption}: {svg: string; caption?: ReactNode}): ReactNode {
  return (
    <figure style={{margin: '1.5rem 0'}}>
      <div
        className="takt-chart"
        tabIndex={0}
        style={{overflowX: 'auto', maxWidth: '100%'}}
        dangerouslySetInnerHTML={{__html: svg}}
      />
      {caption && <figcaption style={{fontSize: '0.9rem', marginTop: '0.6rem'}}>{caption}</figcaption>}
    </figure>
  );
}
