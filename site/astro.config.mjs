import { unified } from '@astrojs/markdown-remark';
import { defineConfig } from 'astro/config';
import remarkCommitLinks from './plugins/remark-commit-links.mjs';
import remarkDropPreamble from './plugins/remark-drop-preamble.mjs';

export default defineConfig({
  site: 'https://cef62.github.io',
  base: '/usher',
  markdown: {
    processor: unified({
      remarkPlugins: [remarkDropPreamble, remarkCommitLinks],
    }),
  },
  vite: {
    // Pages import CHANGELOG.md and the app icon from the repo root, outside
    // site/ (Vite's project root): allow serving them in dev.
    server: { fs: { allow: ['..'] } },
  },
});
