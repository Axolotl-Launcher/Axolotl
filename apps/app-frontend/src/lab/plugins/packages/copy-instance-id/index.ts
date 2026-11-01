import { CopyIcon } from '@modrinth/assets'
import { defineMessage } from '@modrinth/ui'

import { defineLabPlugin } from '@/lab/plugins/types'

import CopyInstanceIdButton from './CopyInstanceIdButton.vue'

export default defineLabPlugin({
    manifest: {
        id: 'copy-instance-id',
        name: defineMessage({
            id: 'app.lab.plugins.copy-instance-id.name',
            defaultMessage: 'Copy instance ID',
        }),
        description: defineMessage({
            id: 'app.lab.plugins.copy-instance-id.description',
            defaultMessage: 'Adds a button to the instance action row that copies its ID.',
        }),
        version: '1.0.0',
        category: 'maintenance',
        icon: CopyIcon,
        permissions: ['clipboard'],
    },
    slots: [
        {
            slot: 'instance.actions',
            component: CopyInstanceIdButton,
            order: 10,
        },
    ],
})
