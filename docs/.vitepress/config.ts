import { defineConfig } from 'vitepress'
import llmstxt from 'vitepress-plugin-llms'

export default defineConfig({
  title: 'daml-tools',
  description:
    'Consumer documentation for daml-tools packages: Daml parser, linter, formatter, and custom-rule plugins.',
  base: '/daml-tools/',
  vite: {
    plugins: [llmstxt()],
  },
  markdown: {
    languages: ['bash', 'json', 'rust', 'sh', 'text', 'toml', 'typescript', 'yaml'],
    languageAlias: {
      daml: 'haskell',
    },
  },
  themeConfig: {
    nav: [
      { text: 'Tutorials', link: '/tutorials/first-run' },
      { text: 'How-to', link: '/how-to/format-daml' },
      { text: 'Reference', link: '/reference/cli' },
      { text: 'Explanation', link: '/explanation/workspace-architecture' },
      {
        text: 'GitHub',
        link: 'https://github.com/stevennevins/daml-tools',
      },
    ],
    sidebar: [
      {
        text: 'Tutorials',
        items: [
          { text: 'First run', link: '/tutorials/first-run' },
          {
            text: 'Build a parser tool',
            link: '/tutorials/build-a-parser-tool',
          },
          {
            text: 'Write a custom lint rule',
            link: '/tutorials/write-a-daml-lint-custom-rule',
          },
        ],
      },
      {
        text: 'How-to guides',
        items: [
          { text: 'Format Daml source', link: '/how-to/format-daml' },
          { text: 'Scan Daml source', link: '/how-to/scan-daml' },
        ],
      },
      {
        text: 'Reference',
        items: [
          { text: 'CLI reference', link: '/reference/cli' },
          { text: 'Crate and package reference', link: '/reference/crates' },
          {
            text: 'Custom rule contract',
            link: '/reference/daml-lint-custom-rule-contract',
          },
        ],
      },
      {
        text: 'Explanation',
        items: [
          {
            text: 'Workspace architecture',
            link: '/explanation/workspace-architecture',
          },
          {
            text: 'Formatter verification model',
            link: '/explanation/formatter-verification',
          },
          {
            text: 'daml-lint rule authoring model',
            link: '/explanation/daml-lint-rule-authoring',
          },
        ],
      },
    ],
    search: {
      provider: 'local',
    },
    socialLinks: [
      {
        icon: 'github',
        link: 'https://github.com/stevennevins/daml-tools',
      },
    ],
    editLink: {
      pattern:
        'https://github.com/stevennevins/daml-tools/edit/main/docs/:path',
    },
    outline: {
      level: [2, 3],
    },
    footer: {
      message: 'Licensed under AGPL-3.0-only',
      copyright: 'Copyright © Steven Nevins',
    },
  },
})
