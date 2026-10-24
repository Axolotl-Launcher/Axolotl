# Copying

Axolotl Launcher's frontend is a modified version of Modrinth App's frontend. It is licensed under the GNU General Public License, Version 3 only, provided in [LICENSE](./LICENSE).

Copyright for the original work remains with Rinth, Inc. and the original contributors. Axolotl modifications are Copyright © 2026 Garbage Human Studio and were developed by Mystic Stars.

Axolotl Launcher is an independent, unofficial client. Modrinth is a trademark of Rinth, Inc. and is referenced only to identify API and file-format compatibility. Axolotl Launcher is not affiliated with or endorsed by Rinth, Inc.

## Schematic preview attribution

The local schematic preview uses [Deepslate](https://github.com/misode/deepslate) 0.26.0 for Minecraft blockstate and model mesh generation. Deepslate is distributed under the MIT License, and its package license is included with the installed dependencies.

The bundled blockstate data, block models, default block properties, and texture atlas are sourced from the public [Misode mcmeta](https://github.com/misode/mcmeta) dataset, which is generated from Minecraft: Java Edition client resources and Mojang's data generator. The current bundle is pinned to `summary@b8170fbc07725bf4930d189ad5dc16f70e09b9cd` and `atlas@a73f0316d9cea52a53381664328bda00e5fe79e4` (Minecraft `26.3-snapshot-6`) and can be reproduced with `scripts/axolotl/sync-schematic-resources.mjs`.

Minecraft and its original resources are Copyright Mojang AB. Their namespaced identifiers are retained only where required to identify compatible game content. Axolotl Launcher is not affiliated with or endorsed by Mojang or Microsoft.

## Recipe generator attribution

The recipe generator's vanilla recipe catalogs and expanded item tags are sourced from [destruc7i0n/crafting](https://github.com/destruc7i0n/crafting) at commit `e6c71dd816216a73cda2787aa5253f641b57fbeb`, which is distributed under the MIT License. A verbatim copy is provided in [third-party/licenses/MIT.txt](../../third-party/licenses/MIT.txt).

Item identifiers, readable names, and textures are sourced from the `minecraft-textures` npm package version `26.2.1` by destruc7i0n, which is distributed under the GNU General Public License, Version 3. The GPL-3.0 text is provided in the app frontend [LICENSE](./LICENSE).

The recipe data and item artwork ultimately derive from Minecraft data generator output and Minecraft game resources. Minecraft and its original resources are Copyright Mojang Studios / Microsoft and are used only to identify compatible game content. Axolotl Launcher is not affiliated with or endorsed by Mojang Studios or Microsoft.

## AI integration attribution

The AI provider settings information architecture, provider catalog, and provider descriptions are adapted from [LobeChat](https://github.com/lobehub/lobe-chat) at commit `a27dfaeda1ab499ac024a6eb0448917b216ba8a1`. The localized provider descriptions under `src/data/lobehub-provider-descriptions` are reproduced from that release. Bundled text-model metadata is synchronized separately from [LobeHub's model bank](https://github.com/lobehub/lobehub/tree/main/packages/model-bank/src/aiModels), with its exact source revision recorded in the backend catalog. LobeChat is distributed under the LobeHub Community License; a verbatim copy of that license is provided in [third-party/licenses/LobeHub-Community-License.txt](../../third-party/licenses/LobeHub-Community-License.txt).

Provider and model marks are loaded from [Lobe Icons](https://github.com/lobehub/lobe-icons) through `@lobehub/icons-static-svg`. Lobe Icons is distributed under the MIT License; its license is included with the installed package.

The translation provider and model selection workflow is adapted from [Read Frog](https://github.com/mengxi-ream/read-frog) at commit `951e20fa82cd96d606a44156362f402c79d11473`. Read Frog is distributed under the GNU General Public License, Version 3; a copy is provided in [third-party/licenses/Read-Frog-GPL-3.0.txt](../../third-party/licenses/Read-Frog-GPL-3.0.txt).

## Local LogAgent attribution

Local crash analysis uses [Deep Agents JS](https://github.com/langchain-ai/deepagentsjs) 1.14.2 with LangChain. These packages are distributed under the MIT License; their licenses are included with the installed dependencies. The browser entry is loaded only when a local analysis starts. Model requests and credentials are handled by the launcher Rust gateway.

Axolotl Launcher's LogAgent is adapted with authorization from [LogShare](https://github.com/anylyralabs/LogShare). Its diagnostic workflow, focused log windows, bounded file tools and result contract are adapted from LogShare's PHP LogAgent. The local implementation omits RAG and external search tools. LogShare's source license credits Aternos GmbH & ZeinkLab (2018–2024). No LogShare server credentials are bundled.
