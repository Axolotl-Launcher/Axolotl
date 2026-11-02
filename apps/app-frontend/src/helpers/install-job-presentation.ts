import { defineMessages, type MessageDescriptor } from '@modrinth/ui'

import type { InstallJobSnapshot } from './install'

const messages = defineMessages({
    install: { id: 'app.downloads.task.install-content', defaultMessage: 'Install content' },
    installNamed: {
        id: 'app.downloads.task.install-content-named',
        defaultMessage: 'Install content: {name}',
    },
    installBatch: {
        id: 'app.downloads.task.install-content-batch',
        defaultMessage: 'Install content: {name} ({count} items)',
    },
    change: { id: 'app.downloads.task.change-content', defaultMessage: 'Change content' },
    changeNamed: {
        id: 'app.downloads.task.change-content-named',
        defaultMessage: 'Change content: {name}',
    },
    changeBatch: {
        id: 'app.downloads.task.change-content-batch',
        defaultMessage: 'Change content: {name} ({count} items)',
    },
})

type FormatMessage = (
    message: MessageDescriptor,
    values?: Record<string, string | number>,
) => string

function resourceNames(job: InstallJobSnapshot): string[] {
    const names = job.resource_names?.filter((name) => name.trim()) ?? []
    // Old jobs did not persist project titles. Keep their real filenames intact.
    return names.length ? names : [...new Set((job.items ?? []).map((item) => item.name))]
}

export function installJobTitle(
    job: InstallJobSnapshot,
    formatMessage: FormatMessage,
    fallback: string,
): string {
    const title = job.display?.title
    const contentJob = job.kind === 'install_content' || job.kind === 'change_content'
    if (contentJob && (!title || title === 'Install resources' || title === 'Change resources')) {
        const names = resourceNames(job)
        const install = job.kind === 'install_content'
        const descriptor = names.length
            ? names.length === 1
                ? install
                    ? messages.installNamed
                    : messages.changeNamed
                : install
                  ? messages.installBatch
                  : messages.changeBatch
            : install
              ? messages.install
              : messages.change
        return formatMessage(descriptor, { name: names[0] ?? '', count: names.length })
    }
    return (
        title ||
        (job.details?.type === 'instance' ? job.details.name : null) ||
        (job.details?.type === 'modpack' ? job.details.title : null) ||
        fallback
    )
}

export function installJobResourceMatches(job: InstallJobSnapshot, query: string): boolean {
    return [...(job.resource_names ?? []), ...(job.items ?? []).map((item) => item.name)].some(
        (name) => name.toLowerCase().includes(query),
    )
}
