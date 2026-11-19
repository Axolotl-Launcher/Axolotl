import { defineMessages } from '@modrinth/ui'

export const networkMessages = defineMessages({
    title: { id: 'app.network.offline.title', defaultMessage: 'Unable to connect' },
    description: {
        id: 'app.network.offline.description',
        defaultMessage:
            'Check your network connection: make sure Wi-Fi is on, your network cable is connected, airplane mode is off, and your proxy settings are correct.',
    },
    settings: {
        id: 'app.network.offline.settings',
        defaultMessage: 'Network connection settings',
    },
    settingsFailed: {
        id: 'app.network.offline.settings-failed',
        defaultMessage: 'Unable to open network settings. Open them from your system settings.',
    },
    retry: { id: 'app.network.offline.retry', defaultMessage: 'Retry' },
    disconnected: { id: 'app.network.offline.disconnected', defaultMessage: 'Disconnected' },
    simulate: {
        id: 'app.settings.developer.simulate-offline',
        defaultMessage: 'Simulate being offline',
    },
    simulateDescription: {
        id: 'app.settings.developer.simulate-offline-description',
        defaultMessage:
            'Show the offline page and connection indicator for this session. Turn this off to restore normal connectivity.',
    },
})
