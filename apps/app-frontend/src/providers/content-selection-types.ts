import type { BrowseInstallPreferences, BrowseSelectedProject } from '@modrinth/ui'
import { createContext, defineMessages } from '@modrinth/ui'
import type { ComputedRef, Ref } from 'vue'

import type ContentInstallPreviewModal from '@/components/ui/ContentInstallPreviewModal.vue'
import type ResourceOperationPreviewModal from '@/components/ui/ResourceOperationPreviewModal.vue'
import type { ContentIdentity } from '@/helpers/content-identity'
import type { GameInstance } from '@/helpers/types'
import type { DownloadManager } from '@/providers/download-manager'

export type ContentSelectionProvider = 'modrinth' | 'curseforge'
export type ContentSelectionType = 'mod' | 'resourcepack' | 'datapack' | 'shader' | 'world'
export type ContentSelectionState = 'idle' | 'validating' | 'reviewing' | 'queueing' | 'error'

export interface ContentSelectionItem {
    key: string
    provider: ContentSelectionProvider
    projectId: string
    providerProjectId: string
    versionId?: string
    contentType: ContentSelectionType
    title: string
    iconUrl?: string | null
    preferences?: BrowseInstallPreferences
    targetInstanceId?: string
    slug?: string | null
    fileName?: string | null
    sha1?: string | null
    identity?: ContentIdentity
    versionPending?: boolean
}

export interface ContentSelectionContext {
    instances: Ref<GameInstance[]>
    targetInstance: Ref<GameInstance | null>
    items: Ref<Map<string, ContentSelectionItem>>
    selectedProjects: ComputedRef<BrowseSelectedProject[]>
    selectedCount: ComputedRef<number>
    state: Ref<ContentSelectionState>
    progress: Ref<{ completed: number; total: number }>
    errorKeys: Ref<Set<string>>
    refreshInstances: (
        preferredId?: string | null,
        isCurrent?: () => boolean,
    ) => Promise<GameInstance | null>
    refreshInstalledIdentities: () => Promise<void>
    setTarget: (instance: GameInstance | null) => void
    add: (item: ContentSelectionItem) => Promise<boolean>
    remove: (key: string) => void
    clear: () => void
    isSelected: (key: string) => boolean
    isInstalledIdentity: (
        provider: ContentSelectionProvider,
        projectId: string,
        slug?: string | null,
    ) => boolean
    isInstalling: (key: string) => boolean
    installSelected: () => Promise<boolean>
    setResourceInstallPreview: (
        modal: InstanceType<typeof ResourceOperationPreviewModal> | null,
    ) => void
    setPreviewModal: (modal: InstanceType<typeof ContentInstallPreviewModal> | null) => void
}

export interface CreateContentSelectionOptions {
    addNotification: (notification: { title: string; type: 'error' }) => void
    handleError: (error: unknown) => void
    downloadManager: DownloadManager
}

export const [injectContentSelection, provideContentSelection] =
    createContext<ContentSelectionContext>('App', 'contentSelection')

export const contentSelectionMessages = defineMessages({
    noTarget: {
        id: 'app.content-selection.no-target',
        defaultMessage: 'Select a target instance before adding content.',
    },
    previewFailed: {
        id: 'app.content-selection.preview-failed',
        defaultMessage: 'Some selected content could not be prepared. Remove it or try again.',
    },
    targetChanged: {
        id: 'app.content-selection.target-changed',
        defaultMessage:
            'The selected content belongs to another instance. Switch back or clear it first.',
    },
    duplicateContent: {
        id: 'app.content-selection.duplicate-content',
        defaultMessage: '{project} is already installed or selected from another source.',
    },
    conflictUnavailable: {
        id: 'app.content-selection.conflict-unavailable',
        defaultMessage: 'Could not verify whether this content duplicates another source.',
    },
})
