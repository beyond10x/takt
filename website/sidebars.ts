import type {SidebarsConfig} from '@docusaurus/plugin-content-docs';

const sidebars: SidebarsConfig = {
  docsSidebar: [
    {
      type: 'category',
      label: 'Start here',
      collapsed: false,
      items: [
        'index',
        'guides/install',
        'getting-started',
        'concepts/glossary',
        'known-limitations',
      ],
    },
    {
      type: 'category',
      label: 'Guides',
      collapsed: true,
      items: [
        'guides/configure-a-run',
        'guides/run-a-story',
        'guides/approve-a-capability',
        'guides/read-a-case',
        'guides/read-a-run-report',
        'guides/run-in-a-container',
        'guides/scenario-run',
        'guides/variety-run',
        'guides/deploy-on-kubernetes',
      ],
    },
    {
      type: 'category',
      label: 'Concepts',
      collapsed: true,
      items: [
        'concepts/protocol-and-case',
        'concepts/one-step-at-a-time',
        'concepts/how-a-run-ends',
        'concepts/capabilities-and-approvals',
        'concepts/evidence-and-the-judge',
        'concepts/architecture',
      ],
    },
    {
      type: 'category',
      label: 'Reference',
      collapsed: true,
      items: [
        'reference/cli',
        'reference/configuration',
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
    {
      type: 'category',
      label: 'Project',
      collapsed: true,
      items: [
        'status',
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
        'concepts/roles',
      ],
    },
  ],
};

export default sidebars;
