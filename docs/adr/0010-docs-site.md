# 0010. A docs site on GitHub Pages

- Status: Accepted
- Date: 2026-11-24
- Departs from: the design spec's v1 non-goal "A docs site" (`docs/superpowers/specs/2026-11-23-usher-design.md` §2)

## Context

v1 shipped (0.1.0 to 0.1.3) with only the README. The owner asked for a proper docs website where
people get instructions and see how usher works. Design: `docs/superpowers/specs/2026-11-24-usher-site-design.md`.

## Options

1. **README only.** No screenshots in context, no download button that follows the latest release.
2. **Port hop's Astro site.** Same owner, same stack, a working Pages pipeline and screenshot tooling.
3. **A docs framework (Starlight and similar).** More dependencies and a look that differs from hop.

## Decision

Option 2. `site/` is an Astro site built with `pnpm docs:build` and deployed by
`.github/workflows/pages.yml` to `https://cef62.github.io/usher`. The release workflow dispatches
`pages.yml` after publishing, so the download button follows each release. `pnpm docs:build` is part
of the quality gate and CI.

## Consequences

- The Usage page must stay in step with the UI and the README; a UI change updates both, and the
  screenshots when they change (`site/scripts/screens/`).
- `build-release.yml`'s publish job needs `actions: write` to dispatch `pages.yml`.
- The owner sets Settings › Pages › Source to "GitHub Actions" once.
