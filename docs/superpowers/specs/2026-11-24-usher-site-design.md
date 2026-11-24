# usher docs site — design

- Status: approved by the owner, 2026-11-24 ("go site")
- Reference: hop's site, `/Volumes/Dev/matteo/hop/site` (owner: "nope" to any change from it)

## Intent

People land on the site to get instructions and see how usher works. It is a reference for users
first; the home page still lets a newcomer download it.

## Approach

Port hop's site. It is the one sensible way: same owner, same stack, a working Pages pipeline, and
the owner wants nothing different. Alternatives (a README-only "site", a docs framework such as
Starlight) were not taken: the first gives no screenshots or download button, the second adds a
dependency and a look that differs from hop for no gain.

## Design

- Astro in `site/`, built with `pnpm docs:build` (`astro build --root site`), deployed to
  GitHub Pages at `https://cef62.github.io/usher` by `.github/workflows/pages.yml` (copied from hop).
  `pages.yml` runs on pushes to `main` that touch `site/**`, `CHANGELOG.md`, `package.json`,
  `pnpm-lock.yaml`, `src-tauri/icons/**` or itself, and on `workflow_dispatch`.
  `build-release.yml`'s publish job ends with `gh workflow run pages.yml --ref main` (as hop), so
  the download button follows each release.
- `pnpm docs:build` joins the quality gate in `CONTRIBUTING.md` and CI.
- Pages:
  - **Home**: one sentence on what usher does; "how it works" in 3 steps (rank your devices,
    plug in, usher switches); the download button for the latest `_aarch64.dmg` (hop's
    `getDownload`, falling back to `/releases/latest`); the Gatekeeper note linking to Usage;
    a panel screenshot.
  - **Usage** (the main page), one section each: Install (Gatekeeper), The panel, Rank devices
    (drag and keyboard), Auto-switch (and the loop-guard pause), Never use and Forget, USB devices
    that change ports, Settings, Updates, Logs and troubleshooting.
  - **Changelog**: `CHANGELOG.md` with commit links (hop's remark plugins).
  - **404**.
- Screenshots: panel, Settings and What's new, light and dark, captured from the real UI with a
  mocked Tauri backend and made-up devices (hop's `site/scripts/screens/`).
- The app icon on the site is `src-tauri/icons/icon.png`.

## Non-goals

Search, a blog, versioned docs, translations.

## Owner prerequisite

Repo Settings › Pages › Source: "GitHub Actions". Until then the deploy job fails; the build job
and the gate still pass.
