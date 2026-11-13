import { defineMessages } from '@modrinth/ui'
import { confirm } from '@tauri-apps/plugin-dialog'

import { approve_java_selection, get_java_selection_info } from './jre'

const messages = defineMessages({
    title: { id: 'app.java.external.title', defaultMessage: 'Use Java from this storage device?' },
    body: {
        id: 'app.java.external.body',
        defaultMessage:
            'This Java installation is on external, network or unrecognized storage. Games cannot start while it is unavailable.\n\n{path}',
    },
    use: { id: 'app.java.external.use', defaultMessage: 'Use this Java' },
    cancel: { id: 'app.java.external.cancel', defaultMessage: 'Cancel' },
})

/** Selection commits share one confirmation; discovery and preview never approve a device. */
export async function confirmJavaSelection<T extends { path?: string }>(
    runtime: T,
    formatMessage: (
        message: { id: string; defaultMessage: string },
        values?: Record<string, string>,
    ) => string,
): Promise<T | null> {
    if (!runtime.path?.trim()) return runtime
    const selection = await get_java_selection_info(runtime.path)
    if (!selection.requires_confirmation) return selection.runtime
    const accepted = await confirm(formatMessage(messages.body, { path: selection.runtime.path }), {
        title: formatMessage(messages.title),
        kind: 'warning',
        okLabel: formatMessage(messages.use),
        cancelLabel: formatMessage(messages.cancel),
    })
    if (!accepted) return null
    return await approve_java_selection(selection.runtime.path, selection.storage.volume_id)
}
