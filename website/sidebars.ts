import type {SidebarsConfig} from '@docusaurus/plugin-content-docs';

const sidebars: SidebarsConfig = {
  docsSidebar: [
    {
      type: 'category',
      label: 'Start here',
      collapsed: false,
      items: ['index', 'getting-started', 'status', 'known-limitations'],
    },
    {
      type: 'category',
      label: 'Concepts',
      collapsed: true,
      items: [
        'concepts/architecture',
        'concepts/protocol-and-case',
        'concepts/one-step-at-a-time',
        'concepts/capabilities-and-approvals',
        'concepts/evidence-and-the-judge',
        'concepts/how-a-run-ends',
        'concepts/roles',
      ],
    },
    {
      type: 'category',
      label: 'Guides',
      collapsed: true,
      items: [
        'guides/install',
        'guides/configure-a-run',
        'guides/run-a-story',
        'guides/approve-a-capability',
        'guides/read-a-case',
        'guides/run-in-a-container',
        'guides/deploy-on-kubernetes',
        'guides/scenario-run',
        'guides/variety-run',
        'guides/read-a-run-report',
      ],
    },
    {
      type: 'category',
      label: 'Self-improvement',
      collapsed: true,
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
      collapsed: true,
      items: [
        'reference/configuration',
        'reference/cli',
        'reference/exit-codes',
        'reference/case-files',
        'reference/events-jsonl',
        'reference/takt-trace',
        'reference/container-image',
        'reference/protocols',
        'reference/protocol-defaults',
        'reference/metrics-fields',
      ],
    },
  ],
};

export default sidebars;
