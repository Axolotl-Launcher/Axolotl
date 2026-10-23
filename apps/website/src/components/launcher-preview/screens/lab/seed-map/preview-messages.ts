import { defineMessages } from '@modrinth/ui/src/composables/i18n.ts'

import zhCN from '~/locales/launcher-preview/lab-seed-map.zh-CN'

import { registerPreviewMessages } from '../../../lib/i18n'

registerPreviewMessages('zh-CN', zhCN)

/** What the web preview's seed map leaves to the desktop app, by limit reason. */
export const seedMapPreviewMessages = defineMessages({
    title: {
        id: 'launcher-preview.seed-map.limit.title',
        defaultMessage: 'Not in the web preview',
    },
    world: {
        id: 'launcher-preview.seed-map.limit.world',
        defaultMessage:
            'The web preview maps seed {seed} on Java {version}. Download Axolotl to map any seed.',
    },
    area: {
        id: 'launcher-preview.seed-map.limit.area',
        defaultMessage:
            "This area is outside the preview's map. Download Axolotl to explore the whole world.",
    },
    option: {
        id: 'launcher-preview.seed-map.limit.option',
        defaultMessage:
            "Other depths and contour lines are outside the preview's map. Download Axolotl to use them.",
    },
    ore: {
        id: 'launcher-preview.seed-map.limit.ore',
        defaultMessage:
            "This ore is not in the preview's scan. Download Axolotl to scan every ore.",
    },
    import: {
        id: 'launcher-preview.seed-map.limit.import',
        defaultMessage: 'Reading seeds from your worlds needs the desktop app.',
    },
    underground: {
        id: 'launcher-preview.seed-map.limit.underground',
        defaultMessage: 'The underground view needs the desktop app.',
    },
    blocks: {
        id: 'launcher-preview.seed-map.limit.blocks',
        defaultMessage: 'Real structure blocks need the desktop app. Frames are still available.',
    },
})
