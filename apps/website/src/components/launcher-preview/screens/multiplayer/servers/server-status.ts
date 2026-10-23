import { defineMessages } from '@modrinth/ui/src/composables/i18n.ts'

import type { ServerStatus } from '../../../lib/server-core.ts'

export const serverStatusMessages = defineMessages({
    notRunning: { id: 'app.servers.status.not-running', defaultMessage: 'Not running' },
    eulaPending: { id: 'app.servers.status.eula-pending', defaultMessage: 'EULA pending' },
    starting: { id: 'app.servers.status.starting', defaultMessage: 'Starting' },
    running: { id: 'app.servers.status.running', defaultMessage: 'Running' },
    crashed: { id: 'app.servers.status.crashed', defaultMessage: 'Crashed' },
})

export interface ServerStatusMeta {
    label: (typeof serverStatusMessages)[keyof typeof serverStatusMessages]
    color: string
}

export const SERVER_STATUS_META: Record<ServerStatus, ServerStatusMeta> = {
    created: { label: serverStatusMessages.notRunning, color: 'text-[var(--color-text-tertiary)]' },
    eula_pending: { label: serverStatusMessages.eulaPending, color: 'text-orange' },
    ready: { label: serverStatusMessages.notRunning, color: 'text-[var(--color-text-tertiary)]' },
    starting: { label: serverStatusMessages.starting, color: 'text-orange' },
    running: { label: serverStatusMessages.running, color: 'text-green' },
    crashed: { label: serverStatusMessages.crashed, color: 'text-red' },
}
