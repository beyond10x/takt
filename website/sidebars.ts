import type {SidebarsConfig} from '@docusaurus/plugin-content-docs';

const sidebars: SidebarsConfig = {
  docsSidebar: [
    {
      type: 'category',
      label: 'Start here',
      collapsed: false,
      items: ['index', 'getting-started', 'status'],
    },
    {
      type: 'category',
      label: 'Concepts',
      collapsed: false,
      items: [
        'concepts/protocol-and-case',
        'concepts/one-step-at-a-time',
        'concepts/evidence-and-the-judge',
        'concepts/roles',
      ],
    },
    {
      type: 'category',
      label: 'Guides',
      collapsed: false,
      items: [
        'guides/run-a-story',
        'guides/scenario-run',
        'guides/variety-run',
        'guides/read-a-run-report',
      ],
    },
    {
      type: 'category',
      label: 'Self-improvement',
      collapsed: false,
      items: [
        'self-improvement/the-loop',
        'self-improvement/scorecard',
        'self-improvement/turns',
        'self-improvement/runs',
        'self-improvement/methodology',
      ],
    },
    {
      type: 'category',
      label: 'Reference',
      collapsed: false,
      items: [
        'reference/cli',
        'reference/exit-codes',
        'reference/events-jsonl',
        'reference/takt-trace',
        'reference/metrics-fields',
        'reference/protocols',
      ],
    },
  ],
};

export default sidebars;
