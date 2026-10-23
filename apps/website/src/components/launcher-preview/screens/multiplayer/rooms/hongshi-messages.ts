import { defineMessages } from '@modrinth/ui/src/composables/i18n.ts'

import type { HongshiErrorType } from './multiplayer.ts'

export const hongshiMessages = defineMessages({
    unsupported: {
        id: 'app.multiplayer.hongshi.unsupported',
        defaultMessage:
            'RedStone Online 2 is not available for this operating system or architecture.',
    },
    build_unavailable: {
        id: 'app.multiplayer.hongshi.error.build-unavailable',
        defaultMessage:
            'The RedStone Online 2 build for this device has not been published yet. Try again later.',
    },
    node_list: {
        id: 'app.multiplayer.hongshi.error.node-list',
        defaultMessage: 'Could not load relay nodes. Check your connection and refresh the list.',
    },
    node_unavailable: {
        id: 'app.multiplayer.hongshi.error.node-unavailable',
        defaultMessage: 'No relay could create a room. Refresh the nodes or choose another relay.',
    },
    invalid_port: {
        id: 'app.multiplayer.hongshi.error.invalid-port',
        defaultMessage: 'Enter a local port between 1 and 65535.',
    },
    install: {
        id: 'app.multiplayer.hongshi.error.install',
        defaultMessage:
            'Could not install the RedStone Online 2 kernel. Check your connection and try again.',
    },
    kernel_start: {
        id: 'app.multiplayer.hongshi.error.kernel-start',
        defaultMessage:
            'The RedStone Online 2 kernel could not start a room. Try again or open the logs.',
    },
    kernel_exit: {
        id: 'app.multiplayer.hongshi.error.kernel-exit',
        defaultMessage:
            'The RedStone Online 2 kernel stopped unexpectedly. Create a new room or open the logs.',
    },
    kernel_output: {
        id: 'app.multiplayer.hongshi.error.kernel-output',
        defaultMessage:
            'The RedStone Online 2 kernel returned an invalid address or output. Open the logs for details.',
    },
    unknown: {
        id: 'app.multiplayer.hongshi.error.unknown',
        defaultMessage: 'Could not create the RedStone Online 2 room. Try again or open the logs.',
    },
})

export function hongshiErrorMessage(type: HongshiErrorType | null | undefined) {
    return hongshiMessages[type ?? 'unknown']
}
