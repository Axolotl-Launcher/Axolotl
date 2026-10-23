import BabricIcon from '@modrinth/assets/icons/tags/loaders/babric.svg?component'
import FabricIcon from '@modrinth/assets/icons/tags/loaders/fabric.svg?component'
import ForgeIcon from '@modrinth/assets/icons/tags/loaders/forge.svg?component'
import LegacyFabricIcon from '@modrinth/assets/icons/tags/loaders/legacy-fabric.svg?component'
import LiteloaderIcon from '@modrinth/assets/icons/tags/loaders/liteloader.svg?component'
import NeoforgeIcon from '@modrinth/assets/icons/tags/loaders/neoforge.svg?component'
import OptifineIcon from '@modrinth/assets/icons/tags/loaders/optifine.svg?component'
import QuiltIcon from '@modrinth/assets/icons/tags/loaders/quilt.svg?component'
import VanillaIcon from '@modrinth/assets/icons/tags/loaders/vanilla.svg?component'
import type { Component } from 'vue'

/** `getLoaderIcon` of `@modrinth/assets`, narrowed to the loaders an instance can use. */
const loaderIcons: Record<string, Component> = {
    babric: BabricIcon,
    fabric: FabricIcon,
    forge: ForgeIcon,
    'legacy-fabric': LegacyFabricIcon,
    liteloader: LiteloaderIcon,
    neoforge: NeoforgeIcon,
    optifine: OptifineIcon,
    quilt: QuiltIcon,
    vanilla: VanillaIcon,
}

export function getLoaderIcon(loader: string | null | undefined): Component | undefined {
    if (!loader) return undefined
    return loaderIcons[loader.toLowerCase()]
}
