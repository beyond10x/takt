import type {Config} from '@docusaurus/types';
import type * as Preset from '@docusaurus/preset-classic';
import path from 'node:path';
import type {LoadContext, Plugin, PluginConfig, RouteConfig} from '@docusaurus/types';
import {withProductSite} from '@beyond10x/docs-system/product-site';
import rehypeLayout from './scripts/rehype-layout.mjs';

const config: Config = {
  title: 'takt',
  tagline:
    'Agentic coding that counts nothing as done until a validator has recorded the evidence.',
  favicon: 'img/mark.svg',

  future: {
    v4: true,
  },

  url: 'https://beyond10x.github.io',
  baseUrl: '/takt/',

  organizationName: 'beyond10x',
  projectName: 'takt',
  trailingSlash: false,

  onBrokenLinks: 'throw',
  onBrokenAnchors: 'throw',

  markdown: {
    format: 'detect',
    hooks: {
      onBrokenMarkdownLinks: 'throw',
    },
  },

  i18n: {
    defaultLocale: 'en',
    locales: ['en'],
  },

  presets: [
    [
      'classic',
      {
        docs: {
          sidebarPath: './sidebars.ts',
          routeBasePath: 'docs',
          editUrl: 'https://github.com/beyond10x/takt/tree/main/website/',
          rehypePlugins: [rehypeLayout],
        },
        blog: false,
        theme: {
          customCss: './src/css/custom.css',
        },
      } satisfies Preset.Options,
    ],
  ],

  themeConfig: {
    navbar: {
      title: 'takt',
      items: [
        {to: '/docs/', label: 'Docs', position: 'left', activeBaseRegex: '^/takt/docs/$'},
        {to: '/docs/concepts/protocol-and-case', label: 'Concepts', position: 'left'},
        {to: '/docs/guides/run-a-story', label: 'Guides', position: 'left'},
        {to: '/docs/self-improvement/the-loop', label: 'Self-improvement', position: 'left'},
        {to: '/docs/reference/cli', label: 'Reference', position: 'left'},
        {to: '/docs/status', label: 'Status', position: 'left'},
        {href: 'pathname:///takt/protocols/', label: 'Protocols', position: 'left'},
        {href: 'https://github.com/beyond10x/takt', label: 'GitHub', position: 'right'},
      ],
    },
    footer: {
      links: [
        {
          title: 'Start here',
          items: [
            {label: 'Overview', to: '/docs/'},
            {label: 'Getting started', to: '/docs/getting-started'},
            {label: 'Status', to: '/docs/status'},
          ],
        },
        {
          title: 'Self-improvement',
          items: [
            {label: 'The loop', to: '/docs/self-improvement/the-loop'},
            {label: 'Runs', to: '/docs/self-improvement/runs'},
            {label: 'Methodology', to: '/docs/self-improvement/methodology'},
          ],
        },
        {
          title: 'Reference',
          items: [
            {label: 'CLI', to: '/docs/reference/cli'},
            {label: 'Exit codes', to: '/docs/reference/exit-codes'},
            {label: 'Protocol viewer', href: 'pathname:///takt/protocols/'},
          ],
        },
        {
          title: 'Project',
          items: [
            {label: 'Source', href: 'https://github.com/beyond10x/takt'},
            {label: 'Releases', href: 'https://github.com/beyond10x/takt/releases'},
            {label: 'Design', href: 'https://github.com/beyond10x/takt/blob/main/docs/design/takt.md'},
          ],
        },
      ],
      copyright: 'A beyond10x project. takt · Apache-2.0.',
    },
  } satisfies Preset.ThemeConfig,
};

// The product layer hands Docusaurus two absolute build paths, the font directory and the landing
// data module, and Docusaurus serializes both into the client bundle, where scan-text refuses them
// as personal paths. Name them relative to the site and through the '@generated' alias instead,
// as the connectors site does.
type PluginFactory = (context: LoadContext, options: unknown) => Plugin;

function portable(site: Config): Config {
  const relative = (directory: string | undefined) =>
    directory && path.isAbsolute(directory)
      ? path.relative(__dirname, directory).split(path.sep).join('/')
      : directory;
  const aliased = (factory: PluginFactory): PluginFactory =>
    function portableModules(context, options) {
      const plugin = factory(context, options);
      const contentLoaded = plugin.contentLoaded?.bind(plugin);
      if (!contentLoaded) return plugin;
      const alias = (module: unknown) =>
        typeof module === 'string' && module.startsWith(context.generatedFilesDir)
          ? '@generated/' + path.relative(context.generatedFilesDir, module).split(path.sep).join('/')
          : module;
      return {
        ...plugin,
        contentLoaded: ({content, actions}) =>
          contentLoaded({
            content,
            actions: {
              ...actions,
              addRoute: (route: RouteConfig) =>
                actions.addRoute({
                  ...route,
                  modules: Object.fromEntries(
                    Object.entries(route.modules ?? {}).map(([name, module]) => [name, alias(module)]),
                  ) as RouteConfig['modules'],
                }),
            },
          }),
      };
    };
  const plugins = (site.plugins ?? []) as unknown[];
  return {
    ...site,
    staticDirectories: (site.staticDirectories ?? ['static']).map(relative) as string[],
    plugins: plugins.map((entry) =>
      Array.isArray(entry) && typeof entry[0] === 'function'
        ? [aliased(entry[0] as PluginFactory), entry[1]]
        : entry,
    ) as PluginConfig[],
  };
}

export default portable(withProductSite(config, {landing: './product.json', mark: 'Tk'}));
