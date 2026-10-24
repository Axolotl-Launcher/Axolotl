import { reactive } from 'vue'

import { previewDomain, type PreviewState } from './state'

export interface PostUpgradeWarning {
    fileName: string
    projectId?: string | null
    versionId?: string | null
}

export interface PostUpgradeNotice {
    targetGameVersion: string
    warnings: PostUpgradeWarning[]
}

/** Notices belong to the preview session and survive navigation between instance tabs. */
export function instanceUpgradeNotices(preview: PreviewState) {
    return previewDomain(preview, 'instance-upgrade-notices', () => {
        const notices = reactive<Record<string, PostUpgradeNotice>>({})
        return {
            value: {
                get: (instanceId: string) => notices[instanceId] ?? null,
                set: (instanceId: string, notice: PostUpgradeNotice) => {
                    notices[instanceId] = notice
                },
                dismiss: (instanceId: string) => {
                    Reflect.deleteProperty(notices, instanceId)
                },
            },
        }
    })
}

export function matchesPostUpgradeWarning(
    warning: PostUpgradeWarning,
    item: { file_name: string; project?: { id: string } | null; version?: { id: string } | null },
) {
    return (
        warning.fileName === item.file_name.replace(/\.disabled$/, '') ||
        (!!warning.projectId &&
            warning.projectId === item.project?.id &&
            (!warning.versionId || warning.versionId === item.version?.id))
    )
}
