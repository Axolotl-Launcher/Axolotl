import type { ServerTypeId } from '../../../lib/server-core.ts'

/** Color, monogram and icon used to badge a server type across cards and the wizard. */
export interface ServerTypeMeta {
    colorVar: string
    monogram: string
    icon?: string
}

/** The app's `assets/instance-icons` artwork, served with the site. */
const typeIcon = (name: string) => `/launcher-preview/server-types/${name}`

const PLATFORM_ID = (id: ServerTypeId) => `var(--color-platform-${id})`

export const SERVER_TYPE_META: Record<ServerTypeId, ServerTypeMeta> = {
    vanilla: { colorVar: 'var(--color-brand)', monogram: 'V', icon: typeIcon('mojang.svg') },
    fabric: { colorVar: PLATFORM_ID('fabric'), monogram: 'F', icon: typeIcon('fabric.webp') },
    paper: { colorVar: PLATFORM_ID('paper'), monogram: 'P', icon: typeIcon('paper.svg') },
    forge: { colorVar: PLATFORM_ID('forge'), monogram: 'Fo', icon: typeIcon('forge.webp') },
    neoforge: { colorVar: PLATFORM_ID('neoforge'), monogram: 'N', icon: typeIcon('neoforge.webp') },
    quilt: { colorVar: PLATFORM_ID('quilt'), monogram: 'Q' },
}
