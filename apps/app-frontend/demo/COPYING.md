# Copying

This directory builds a web demo of Axolotl Launcher's frontend. The demo's code is part of the frontend and is covered by its [COPYING](../COPYING.md) and [LICENSE](../LICENSE).

## Third-party content shown in the demo

The recorded data and images in `data/` and `assets/` are not covered by that license. They are included only so the launcher's screens can be shown with realistic content, and are not offered for reuse. Copyright remains with their owners:

- `data/news.json` and `assets/news/`: Minecraft news articles and their images, recorded from Minecraft.net. They are Copyright Mojang Studios / Microsoft.
- `data/discover/`, `data/projects/`, `data/modpacks.json`, `data/instance-content.json`, `data/screenshots.json`, `assets/projects/` and `assets/screenshots/`: search results, project pages (descriptions, versions and team members), modpack file lists, version data, icons and gallery images (shown as the instances' screenshots) of projects published on Modrinth. Each project belongs to its authors under the license shown on its Modrinth page.
- `assets/screenshots/fabulously-optimized/ancient-city.webp` and `assets/warden/`: a screenshot of Minecraft taken for the demo, the world it shows (baked from the save it was taken in, with Minecraft's block textures) and the warden's, sculk's and particles' textures and sounds from Minecraft. They are Copyright Mojang Studios / Microsoft.
- `data/content-names.json`: Chinese names and MC 百科 (mcmod.cn) entry IDs, taken from the dictionary the launcher bundles.
- `data/tags.json`: game version, loader, category and license tags, including their icons, recorded from the Modrinth API.
- `data/servers/`: console output and configuration files recorded from Minecraft's dedicated server and Fabric's server launcher, which belong to Mojang Studios / Microsoft and the Fabric project.
- `data/default-skins.json` and `assets/skins/`: the skins the launcher bundles. Minecraft's default and skin pack skins are Copyright Mojang Studios / Microsoft; the Mr. Pack pride skins belong to Rinth, Inc.
- `data/account.json` and `assets/skins/mystic-stars-*.png`: the public Minecraft profile, skin and cape of Mystic_Stars, retrieved from Mojang's profile and texture services. The skin belongs to its author; the cape is Copyright Mojang Studios / Microsoft.
- `data/remote-servers.json` and `assets/servers/`: recorded server-list responses and icons of hjfunnyMC and Hypixel. Their names, icons and other content belong to their respective owners.

Images in project descriptions, gallery images and member avatars are not stored here: the demo shows them from their original addresses, as the launcher does.

[data/sources.json](data/sources.json) lists where and when each recording was made.

The demo is not affiliated with or endorsed by Mojang Studios, Microsoft, Rinth, Inc. or the authors of the projects shown. Owners who want their content removed can open an issue, and it will be taken out of the demo.
