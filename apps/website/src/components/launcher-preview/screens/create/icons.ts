import type { MessageDescriptor } from '@modrinth/ui/src/composables/i18n.ts'

import {
    builtInInstanceIcons,
    getLoaderInstanceIcon,
    modrinth3DInstanceIcons,
} from '../../lib/instance-icons'
import { iconBackgroundMessages } from './messages'

export { builtInInstanceIcons, getLoaderInstanceIcon, modrinth3DInstanceIcons }

/** `getLoaderInstanceIcon(loader)?.url`, the file picker's `getLoaderInstanceIconUrl`. */
export function getLoaderInstanceIconUrl(loader: string | null | undefined): string | null {
    return getLoaderInstanceIcon(loader)?.url ?? null
}

export interface IconBackground {
    id: string
    name: MessageDescriptor
    colors: [string, string] | null
    style: Record<string, string>
}

function background(
    id: string,
    name: MessageDescriptor,
    top: string,
    bottom: string,
): IconBackground {
    return {
        id,
        name,
        colors: [top, bottom],
        style: { backgroundImage: `linear-gradient(145deg, ${top}, ${bottom})` },
    }
}

/** The picker's backgrounds (`InstanceIconPickerModal.vue`). */
export const iconBackgrounds: IconBackground[] = [
    {
        id: 'transparent',
        name: iconBackgroundMessages.transparent,
        colors: null,
        style: {
            backgroundColor: '#ffffff',
            backgroundImage:
                'linear-gradient(45deg, #d7d9de 25%, transparent 25%), linear-gradient(-45deg, #d7d9de 25%, transparent 25%), linear-gradient(45deg, transparent 75%, #d7d9de 75%), linear-gradient(-45deg, transparent 75%, #d7d9de 75%)',
            backgroundPosition: '0 0, 0 8px, 8px -8px, -8px 0',
            backgroundSize: '16px 16px',
        },
    },
    background('grass', iconBackgroundMessages.grass, '#7fc95b', '#2f7d32'),
    background('ocean', iconBackgroundMessages.ocean, '#55b7ff', '#3157c8'),
    background('amethyst', iconBackgroundMessages.amethyst, '#c084fc', '#6d28d9'),
    background('sunset', iconBackgroundMessages.sunset, '#ffba52', '#e94b64'),
    background('cherry', iconBackgroundMessages.cherry, '#ff9ec4', '#b93670'),
    background('nether', iconBackgroundMessages.nether, '#ef5a46', '#6d1717'),
    background('slime', iconBackgroundMessages.slime, '#b6ee55', '#44972f'),
    background('deep-dark', iconBackgroundMessages.deepDark, '#245369', '#0c1f2b'),
    background('stone', iconBackgroundMessages.stone, '#aeb5bd', '#525b66'),
    background('midnight', iconBackgroundMessages.midnight, '#4d5f8f', '#171b2d'),
]
