import type { ServerTypeId } from '@modrinth/server'

/** Color, monogram and icon used to badge a server type across cards and the wizard. */
export interface ServerTypeMeta {
    colorVar: string
    monogram: string
    icon?: string
}

const typeIcon = (name: string) =>
    new URL(`../../../assets/instance-icons/${name}`, import.meta.url).href

const PLATFORM_ID = (id: ServerTypeId) => `var(--color-platform-${id})`

export const SERVER_TYPE_META: Record<ServerTypeId, ServerTypeMeta> = {
    vanilla: { colorVar: 'var(--color-brand)', monogram: 'V', icon: typeIcon('Mojang.svg') },
    fabric: { colorVar: PLATFORM_ID('fabric'), monogram: 'F', icon: typeIcon('Fabric.png') },
    paper: { colorVar: PLATFORM_ID('paper'), monogram: 'P', icon: typeIcon('Paper.svg') },
    forge: { colorVar: PLATFORM_ID('forge'), monogram: 'Fo', icon: typeIcon('Forge.jpeg') },
    neoforge: { colorVar: PLATFORM_ID('neoforge'), monogram: 'N', icon: typeIcon('NeoForge.png') },
    quilt: { colorVar: PLATFORM_ID('quilt'), monogram: 'Q' },
}
