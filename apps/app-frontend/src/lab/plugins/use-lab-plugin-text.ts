import { useVIntl } from '@modrinth/ui'

import type { LabPluginText } from './types'

/**
 * Resolves plugin-supplied text. A message descriptor goes through the
 * launcher's translation layer; a plain string is shown as written.
 */
export function useLabPluginText(): (text: LabPluginText) => string {
    const { formatMessage } = useVIntl()
    return (text: LabPluginText) => (typeof text === 'string' ? text : formatMessage(text))
}
