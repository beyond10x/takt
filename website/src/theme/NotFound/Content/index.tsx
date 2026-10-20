import type {ReactNode} from 'react';
import clsx from 'clsx';
import Head from '@docusaurus/Head';
import Link from '@docusaurus/Link';
import {PageMetadata} from '@docusaurus/theme-common';
import Heading from '@theme/Heading';
import type {Props} from '@theme/NotFound/Content';

const PLACES = [
  {to: '/docs', label: 'Overview', what: 'what takt is and what a run does'},
  {to: '/docs/getting-started', label: 'Getting started', what: 'install takt and run one validated step'},
  {to: '/docs/reference/cli', label: 'CLI reference', what: 'every command and its options'},
  {to: '/docs/status', label: 'Status', what: 'what exists today'},
  {to: '/', label: 'Home', what: 'the front page'},
];

// The not-found page. Docusaurus renders this content on its own, without the page around it,
// for an unknown path under /docs/, so the title, the description and the noindex are set here:
// there the tab title fell back to the bare site title. GitHub Pages answers 404.html itself
// with 200; noindex keeps it out of search results.
export default function NotFoundContent({className}: Props): ReactNode {
  return (
    <>
      <PageMetadata
        title="Page not found"
        description="There is no page at this address on the takt site. The overview, getting started, the CLI reference and the status page are linked from it."
      />
      <Head>
        <meta name="robots" content="noindex" />
      </Head>
      <main className={clsx('container margin-vert--xl takt-not-found', className)}>
        <div className="row">
          <div className="col col--6 col--offset-3">
            <Heading as="h1" className="hero__title">
              Page not found
            </Heading>
            <p>There is no page at this address. It may have moved; one of these is a place to start again.</p>
            <ul>
              {PLACES.map(({to, label, what}) => (
                <li key={to}>
                  <Link to={to}>{label}</Link>: {what}
                </li>
              ))}
            </ul>
          </div>
        </div>
      </main>
    </>
  );
}
