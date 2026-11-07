import type { Labrinth } from '@modrinth/api-client'
import type { MessageDescriptor } from '@modrinth/ui'

import { get_project_many } from '@/helpers/cache.js'
import { get_content_items } from '@/helpers/instance'

import {
    dependenciesInstalledMessage,
    dependenciesInstalledTitleMessage,
    dependenciesSkippedMessage,
    dependenciesSkippedTitleMessage,
    skippedReasonMessages,
} from './content-install-messages'

export interface ContentInstallNotifier {
    (instanceId: string, dependencyProjectIds: string[]): Promise<void>
}

type MessageFormatPrimitiveValue = string | number | boolean | null | undefined

type FormatMessage = (
    descriptor: MessageDescriptor,
    values?: Record<string, MessageFormatPrimitiveValue | Date>,
) => string

/**
 * Install-flow toasts that only need i18n + notification sink.
 */
export function createContentInstallDepNotifications(options: {
    formatMessage: FormatMessage
    addNotification: (notification: {
        title: string
        text?: string
        type?: 'error' | 'warning' | 'success' | 'info'
        supportData?: Record<string, unknown>
    }) => void
}) {
    const { formatMessage, addNotification } = options

    async function notifyInstalledDependencies(instanceId: string, dependencyProjectIds: string[]) {
        if (dependencyProjectIds.length === 0) return
        const items = await get_content_items(instanceId).catch(() => [])
        const names = dependencyProjectIds
            .map((id) => {
                const curseForge = id.startsWith('curseforge:')
                const rawId = curseForge ? id.slice('curseforge:'.length) : id
                const item = items.find((candidate) =>
                    candidate.provider_refs.some(
                        (reference) =>
                            reference.provider === (curseForge ? 'curseforge' : 'modrinth') &&
                            String(reference.project_id) === rawId,
                    ),
                )
                return item?.project?.title ?? item?.file_name
            })
            .filter((name): name is string => !!name)
        if (names.length === 0) return
        const list = names.length > 5 ? `${names.slice(0, 5).join(', ')}, …` : names.join(', ')
        addNotification({
            title: formatMessage(dependenciesInstalledTitleMessage),
            text: formatMessage(dependenciesInstalledMessage, {
                count: names.length,
                list,
            }),
            type: 'success',
        })
    }

    async function notifySkippedPlanDependencies(
        skipped: Array<{ project_id: string; reason: string }>,
    ) {
        if (skipped.length === 0) return
        const projectIds = [...new Set(skipped.map((item) => item.project_id).filter((id) => !!id))]
        const projects = await get_project_many(projectIds).catch(
            () => [] as Labrinth.Projects.v2.Project[],
        )
        const projectsById = new Map(projects.map((candidate) => [candidate.id, candidate]))
        const names = skipped.map((item) => {
            const title = projectsById.get(item.project_id)?.title ?? item.project_id
            const reason = formatMessage(
                skippedReasonMessages[item.reason as keyof typeof skippedReasonMessages] ??
                    skippedReasonMessages.already_installed,
            )
            return `${title} (${reason})`
        })
        const list = names.slice(0, 5).join(', ') + (names.length > 5 ? ', …' : '')
        addNotification({
            title: formatMessage(dependenciesSkippedTitleMessage),
            text: formatMessage(dependenciesSkippedMessage, { list }),
            type: 'info',
        })
    }

    return {
        notifyInstalledDependencies,
        notifySkippedPlanDependencies,
    }
}
