import type { Labrinth } from '@modrinth/api-client'
import type { ContentInstallInstance, ContentInstallProjectInfo, ContentItem } from '@modrinth/ui'
import { createContext } from '@modrinth/ui'
import type { Ref } from 'vue'

import type { ModpackInstallModalData } from '@/components/ui/modal/ModpackInstallModal.vue'
import type ResourceOperationPreviewModal from '@/components/ui/ResourceOperationPreviewModal.vue'
import type { CurseForgeManualDownloadImport } from '@/helpers/curseforge'
import type { CurseForgeManualDownloadItem } from '@/helpers/curseforge-manual'

export type ContentInstallCallback = (versionId?: string, installedProjectIds?: string[]) => void

export interface ModalRef {
    show: (initialVersionId?: string) => void
    hide: () => void
}

export interface ModpackInstallModalRef {
    show: (data: ModpackInstallModalData) => void
}

export interface CurseForgeManualDownloadsModalRef {
    show: (payload: {
        items: CurseForgeManualDownloadItem[]
        installed?: number
        instanceId?: string | null
    }) => void
}

export type InstallProvider = 'modrinth' | 'curseforge'
export type ContentInstallTargetMode = 'content' | 'world'

export type ContentInstallInstanceEvent = {
    event: string
    instance_id: string
    project_ids?: string[]
    dependency_project_ids?: string[]
    message?: string
}

export interface ContentInstallContext {
    instances: Ref<ContentInstallInstance[]>
    compatibleLoaders: Ref<string[]>
    gameVersions: Ref<string[]>
    loading: Ref<boolean>
    defaultTab: Ref<'existing' | 'new'>
    preferredLoader: Ref<string | null>
    preferredGameVersion: Ref<string | null>
    releaseGameVersions: Ref<Set<string>>
    projectInfo: Ref<ContentInstallProjectInfo | null>
    symlinkTarget: Ref<string | null | undefined>
    handleInstallToInstance: (instance: ContentInstallInstance) => Promise<void>
    handleCreateAndInstall: (data: {
        name: string
        iconPath: string | null
        iconPreviewUrl: string | null
        loader: string
        gameVersion: string
    }) => Promise<void>
    handleNavigate: (instance: ContentInstallInstance) => void
    handleCancel: () => void
    setResourceInstallPreview: (
        modal: InstanceType<typeof ResourceOperationPreviewModal> | null,
    ) => void
    setContentInstallModal: (ref: ModalRef) => void
    setModpackInstallModal: (ref: ModpackInstallModalRef) => void
    handleModpackInstall: (versionId: string, name: string) => Promise<void>
    handleModpackInstallCancel: () => void
    setCurseForgeManualDownloadsModal: (ref: CurseForgeManualDownloadsModalRef) => void
    showCurseForgeManualDownloads: (
        instanceId: string,
        items: CurseForgeManualDownloadItem[],
    ) => void
    handleCurseForgeManualDownloadsImported: (
        instanceId: string,
        imported: CurseForgeManualDownloadImport[],
    ) => void
    setIncompatibilityWarningModal: (ref: ModalRef) => void
    incompatibilityWarningVersions: Ref<Labrinth.Versions.v2.Version[]>
    incompatibilityWarningCurrentGameVersion: Ref<string>
    incompatibilityWarningCurrentLoader: Ref<string>
    incompatibilityWarningProjectType: Ref<string | undefined>
    incompatibilityWarningProjectIconUrl: Ref<string | undefined>
    incompatibilityWarningProjectName: Ref<string | undefined>
    incompatibilityWarningMessage: Ref<string | undefined>
    incompatibilityWarningInstalling: Ref<boolean>
    handleIncompatibilityWarningInstall: (version: Labrinth.Versions.v2.Version) => Promise<void>
    handleIncompatibilityWarningCancel: () => void
    install: (
        projectId: string,
        versionId?: string | null,
        instanceId?: string | null,
        source?: string,
        callback?: ContentInstallCallback,
        createInstanceCallback?: (instanceId: string) => void,
        hints?: {
            preferredLoader?: string
            preferredGameVersion?: string
            showProjectInfo?: boolean
        },
    ) => Promise<void>
    installCurseForge: (
        projectId: string,
        versionId?: string | null,
        instanceId?: string | null,
        source?: string,
        callback?: ContentInstallCallback,
        createInstanceCallback?: (instanceId: string) => void,
        hints?: {
            preferredLoader?: string
            preferredGameVersion?: string
            showProjectInfo?: boolean
        },
    ) => Promise<void>
    installCurseForgeWorld: (
        projectId: string | number,
        fileId?: string | number | null,
        instanceId?: string | null,
        source?: string,
        callback?: ContentInstallCallback,
    ) => Promise<void>
    installingItems: Ref<Map<string, ContentItem[]>>
    pendingManualDownloadsByInstance: Ref<Map<string, CurseForgeManualDownloadItem[]>>
    installRevisionByInstance: Ref<Map<string, number>>
    installFailureRevisionByInstance: Ref<Map<string, number>>
}

export const [injectContentInstall, provideContentInstall] = createContext<ContentInstallContext>(
    'root',
    'contentInstall',
)
