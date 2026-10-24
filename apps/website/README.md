# Axolotl Website

The official website for Axolotl Launcher.

## Download sources

The browser reads release metadata only from
`https://update.axlmc.org/api/downloads/latest?channel=release|beta`. Switching
download sources keeps that catalog and its version tag, attachment names and
relative paths; it only changes the generated file URLs. No Netlify Functions or
GitHub/GitLink release-list requests are used.
The Update Server download source serves files from
`https://dist.axlmc.org/<relative_path>` while keeping metadata on Update Server.

Sources are registered in `src/utils/download-sources.ts` with an identity,
localized label and description, and an asset URL builder. The settings list and
preference validation derive from that registry; its first entry is the
recommended source used by Automatic. `use-launcher-downloads.ts` owns the single
browser request, loading/error state, timeout and retry. Add a registry entry and
its Chinese translations to introduce a new file source.

## Development

From the repository root:

```sh
pnpm install --frozen-lockfile
pnpm website:dev
```

Create the static production output with `pnpm website:build`.

## Release deployment

Both beta and stable `v*` tags run the `publish-website` job in
`.github/workflows/axolotl-release.yml` after every installer has been built,
verified, and published. The job:

1. Checks out the release tag for the announcement source and `main` for the website.
2. Generates `releases/catalog.json` from the tagged launcher `catalog.ts`.
   Stable releases also update `releases/latest.json`; beta releases preserve it.
3. Installs dependencies and runs `nuxi generate` on GitHub's runner, then checks
   that the released version appears in the prerendered changelog HTML.
4. Saves the static output as a GitHub artifact and commits the generated release
   data to `main` with `[skip ci]`.
5. Uploads `apps/website/.output/public` directly to the
   existing production site using pinned Netlify CLI with `--prod --no-build`.

Netlify hosts the uploaded output without running a hosted build. Deployment
and bandwidth remain subject to the site's plan.

### One-time setup

- Add repository Actions secrets `NETLIFY_AUTH_TOKEN` (a Netlify access token
  with deployment access) and `NETLIFY_SITE_ID` (the existing site's Project ID).
- In Netlify, open **Project configuration > Developer settings > Continuous
  deployment > Build settings**, then set **Build status** to **Stopped builds**.
  This stops Git-triggered builds while still allowing CLI uploads. Do not lock
  production publishing. See [Netlify's build-status documentation](https://docs.netlify.com/build/configure-builds/stop-or-activate-builds/).
- Ensure repository branch rules permit the release workflow's `GITHUB_TOKEN`
  to push generated release data to `main`.
- Any website build-time environment variables previously configured only in
  Netlify must also be provided to the GitHub build step.

Push these workflow changes before creating the next release tag. No credentials
belong in source control. A missing secret or failed website build fails the
website job without replacing the live site. After correcting a failure, rerun
the failed job in GitHub Actions; rebuilding installers is unnecessary.

With Netlify builds stopped, ordinary website commits no longer deploy through
Netlify's Git integration. They go live with the next launcher release, or can be
built and uploaded using the manual workflow below.

## Manual website deployment

Push `.github/workflows/deploy-website.yml` to `main` first. In the GitHub
repository, open **Actions > Deploy website to Netlify > Run workflow**, select
**main**, and click **Run workflow**. Alternatively, run:

```sh
gh workflow run deploy-website.yml --ref main
```

This workflow builds the latest `main` after obtaining the same production
deployment concurrency lock used by the release workflow. Other branches are
skipped. It uses the existing `apps/website/releases/catalog.json` synchronized
by successful releases; it does not regenerate announcements from potentially
unreleased `catalog.ts` entries or build launcher installers. It requires the
same two Netlify secrets and deploys directly to production with `--no-build`.
If installation, building, or static output verification fails, upload is skipped.
