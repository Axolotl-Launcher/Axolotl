import { defineMessage } from '@modrinth/ui'

import { defineLabPlugin } from '@/lab/plugins/types'

import MotdPreviewTool from './MotdPreviewTool.vue'

const name = defineMessage({
    id: 'app.lab.plugins.motd-preview.name',
    defaultMessage: 'MOTD preview',
})

const description = defineMessage({
    id: 'app.lab.plugins.motd-preview.description',
    defaultMessage: 'Preview a server message of the day the way Minecraft renders it.',
})

export default defineLabPlugin({
    manifest: {
        id: 'motd-preview',
        name,
        description,
        version: '1.0.0',
        category: 'creation',
    },
    tool: {
        title: name,
        description,
        component: MotdPreviewTool,
    },
})
