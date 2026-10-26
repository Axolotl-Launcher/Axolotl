# Axolotl Website

The official website for Axolotl Launcher.

## Download sources

The browser reads release metadata only from
`https://update.axlmc.org/api/downloads/latest?channel=release|beta`. Switching
download sources keeps that catalog and its version tag, attachment names and
relative paths; it only changes the generated file URLs. No Pages Functions or
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
5. Uploads `apps/website/.output/public` directly to Cloudflare Pages using pinned
   Wrangler CLI with `--branch main`.

Pages serves only static assets; no Worker adapter or Pages Functions are needed.
GitHub Actions owns the build and publication order. Pages does not run a hosted build.

### One-time setup

- Create a **Pages Direct Upload** project named `axolotl-website` in the account
  that owns `axlmc.org`. Set its production branch to `main`. This is a Pages
  project, separate from the existing `axolotl` Worker. Direct Upload projects
  cannot later switch to Pages Git integration without creating a new project.
- Reuse the repository Actions secret `CLOUDFLARE_API_TOKEN`, shared with the
  Telemetry Worker. Include **Account > Cloudflare Pages > Edit** permission
  for the owning account alongside its existing service permissions.
- Add the repository Actions secret `CLOUDFLARE_ACCOUNT_ID` (the owning
  Cloudflare account ID) and the variable `CLOUDFLARE_PAGES_PROJECT_NAME` (`axolotl-website`).
  The workflow maps the shared token and account secrets to Wrangler's
  `CLOUDFLARE_API_TOKEN` and `CLOUDFLARE_ACCOUNT_ID` environment variables.
- After the first successful upload, associate `axlmc.org` and `www.axlmc.org`
  under **Pages > Custom domains**, then confirm the DNS changes. Keep the
  canonical origin `https://axlmc.org` and redirect `www` to the apex using a
  Cloudflare Redirect Rule. Check HTTPS, all four pages and a missing path before
  retiring the previous host. Domain routing changes do not belong in the CI token.
- Ensure repository branch rules permit the release workflow's `GITHUB_TOKEN`
  to push generated release data to `main`.
- Any website build-time environment variables must be provided to the GitHub
  build step; runtime Pages variables cannot change already generated HTML.

See Cloudflare's [Direct Upload CI guide](https://developers.cloudflare.com/pages/how-to/use-direct-upload-with-continuous-integration/)
and [custom domain setup](https://developers.cloudflare.com/pages/configuration/custom-domains/).

Push these workflow changes before creating the next release tag. No credentials
belong in source control. A missing secret or failed website build fails the
website job without replacing the live site. After correcting a failure, rerun
the failed job in GitHub Actions; rebuilding installers is unnecessary.

Ordinary website commits go live with the next launcher release, or can be built
and uploaded using the manual workflow below.

## Manual website deployment

Push `.github/workflows/deploy-website.yml` to `main` first. In the GitHub
repository, open **Actions > Deploy website to Cloudflare Pages > Run workflow**, select
**main**, and click **Run workflow**. Alternatively, run:

```sh
gh workflow run deploy-website.yml --ref main
```

This workflow builds the latest `main` after obtaining the same production
deployment concurrency lock used by the release workflow. Other branches are
skipped. It uses the existing `apps/website/releases/catalog.json` synchronized
by successful releases; it does not regenerate announcements from potentially
unreleased `catalog.ts` entries or build launcher installers. It requires the
same Pages secret and two Actions variables, and uploads to the production branch `main`.
If installation, building, or static output verification fails, upload is skipped.

To roll back site content, select a previous successful production deployment in
Pages and choose **Rollback**. This does not roll back GitHub's generated release
catalog; the next workflow run publishes the current `main` again.
