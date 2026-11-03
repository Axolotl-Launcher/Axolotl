# Copying

This directory builds a web demo of Axolotl Launcher's frontend. The demo's code is part of the frontend and is covered by its [COPYING](../COPYING.md) and [LICENSE](../LICENSE).

## Third-party content shown in the demo

The recorded data and images in `data/` and `assets/` are not covered by that license. They are included only so the launcher's screens can be shown with realistic content, and are not offered for reuse. Copyright remains with their owners:

- `data/news.json` and `assets/news/`: Minecraft news articles and their images, recorded from Minecraft.net. They are Copyright Mojang Studios / Microsoft.
- `data/discover/`, `data/projects/`, `data/modpacks.json` and `assets/projects/`: search results, project pages (descriptions, versions and team members), version data and icons of projects published on Modrinth. Each project belongs to its authors under the license shown on its Modrinth page.
- `data/content-names.json`: Chinese names and MC 百科 (mcmod.cn) entry IDs, taken from the dictionary the launcher bundles.
- `data/tags.json`: game version, loader, category and license tags, including their icons, recorded from the Modrinth API.

Images in project descriptions, gallery images and member avatars are not stored here: the demo shows them from their original addresses, as the launcher does.

[data/sources.json](data/sources.json) lists where and when each recording was made.

The demo is not affiliated with or endorsed by Mojang Studios, Microsoft, Rinth, Inc. or the authors of the projects shown. Owners who want their content removed can open an issue, and it will be taken out of the demo.
