import type { Labrinth } from '@modrinth/api-client'
import type { Component } from 'vue'
import type { RouteLocationRaw } from 'vue-router'

import type { Option as OverflowMenuOption } from '#ui/components/base/OverflowMenu.vue'

export type ContentCardProject = Pick<
    Labrinth.Projects.v2.Project,
    'id' | 'slug' | 'title' | 'icon_url'
> & {
    license?: Labrinth.Projects.v2.Project['license'] | null
    description?: string
}

export type ContentCardVersion = Pick<Labrinth.Versions.v2.Version, 'id' | 'version_number'> & {
    file_name: string
    date_published?: string
}

export interface ContentOwner {
    id: string
    name: string
    avatar_url?: string
    type: 'user' | 'organization'
    link?: string | RouteLocationRaw | (() => void)
}

export interface ContentRowInlineAction {
    id: string
    label: string
    icon: Component
    action: () => void
}

export interface ContentWorldGroupMeta {
    icon_url?: string | null
    title?: string
    last_played?: string
    game_mode?: string
    hardcore?: boolean
}

export interface ContentCardTableItem {
    id: string
    project: ContentCardProject
    projectLink?: string | RouteLocationRaw
    version?: ContentCardVersion
    versionLink?: string | RouteLocationRaw
    owner?: ContentOwner
    enabled?: boolean
    disabled?: boolean
    disabledTooltip?: string | null
    postUpgradeWarningTooltip?: string | null
    compatibilityWarningTooltip?: string | null
    toggleDisabled?: boolean
    toggleDisabledTooltip?: string | null
    installing?: boolean
    hasUpdate?: boolean
    /** File name that would be restored by the rollback action, when the item
     * has an update backup (`{active}_{previous}.old`) available. */
    rollbackFileName?: string
    hideSwitchVersion?: boolean
    pendingManualDownload?: boolean
    /** Number of installed copies of this online project, when more than one is present. */
    duplicateCount?: number
    localPath?: string
    installationSource?: string
    groupStatus?: string
    updateLabel?: string
    instanceFileId?: string
    instanceEntryId?: string
    instanceResourceId?: string
    instanceMemberId?: string
    instanceOwnershipKind?: 'pack_managed' | 'user_added' | 'local_discovered'
    instanceMaterializationState?: 'present' | 'pending_manual' | 'missing' | 'removed'
    instanceOverrideKind?: 'none' | 'disabled' | 'removed' | 'version'
    instanceCapabilities?: {
        canToggle: boolean
        canDelete: boolean
        canUpdate: boolean
        canChangeVersion: boolean
        canRestorePackDefault: boolean
    }
    dependency?: {
        autoDependency: boolean
        requiredBy: Array<{
            provider: 'modrinth' | 'curseforge' | 'local'
            projectId: string
            releaseId: string
        }>
        requires: Array<{
            provider: 'modrinth' | 'curseforge' | 'local'
            projectId: string
            releaseId: string
        }>
        orphaned: boolean
    } | null
    dependencyBadge?: {
        autoDependency: boolean
        orphaned: boolean
    } | null
    overflowOptions?: OverflowMenuOption[]
    inlineActions?: ContentRowInlineAction[]
    isGroupHeader?: boolean
    group?: string
    groupDepth?: number
    groupItemCount?: number
    groupSwitchVersion?: () => void
    groupChildIds?: string[]
    isGroupChild?: boolean
    groupKind?: 'folder' | 'world' | 'duplicate'
    groupMeta?: ContentWorldGroupMeta
    downloads?: number | null
    followers?: number | null
    categories?: ContentModpackCardCategory[]
}

export type ContentCardTableSortColumn = 'project' | 'version'
export type ContentCardTableSortDirection = 'asc' | 'desc'

export interface BulkOperationStatus {
    message?: string
    progress?: number
    total?: number
    waiting?: boolean
}

/** Content item returned from the app backend API - maps to ContentCardTableItem for display */
export interface ContentItem extends Omit<
    ContentCardTableItem,
    'id' | 'projectLink' | 'disabled' | 'overflowOptions'
> {
    id: string
    file_name: string
    file_path?: string
    size?: number
    project_type: string
    /** Provider-qualified update returned by the launcher backend. */
    resourceUpdate?: { project: { source: string; projectId: string }; releaseId: string } | null
    update:
        | {
              provider: 'modrinth'
              project_id: string
              current_version_id: string
              target_version_id: string
          }
        | {
              provider: 'curseforge'
              project_id: number
              current_file_id: number
              target_file_id: number
          }
        | null
    origin_provider: 'modrinth' | 'curseforge' | 'mcarchive' | 'planet_minecraft' | null
    /** Actual installed artifact source; recognition counterparts do not change it. */
    installedSource?: string | null
    compatibility?: {
        gameVersion: 'compatible' | 'incompatible' | 'unknown'
        loader: 'compatible' | 'incompatible' | 'unknown'
        reasons: string[]
    } | null
    date_added?: string
    environment?: string
    /** Local content source kind (local / curseforge / modrinth_modpack / server_project / imported_modpack / shared_instance). */
    source_kind?: string
    /** True when the file is not linked to any online project. */
    external?: boolean
    /** Loader derived from the installed version or locally parsed mod metadata. */
    loader?: string
    installing?: boolean
    pendingManualDownload?: boolean
    rollback?: { file_name: string } | null
    provider_refs: Array<
        | {
              provider: 'modrinth'
              project_id: string
              version_id: string | null
          }
        | {
              provider: 'curseforge'
              project_id: number
              file_id: number | null
          }
    >
}

export type ContentModpackCardProject = Pick<
    Labrinth.Projects.v2.Project,
    'id' | 'slug' | 'title' | 'icon_url' | 'description'
> & {
    downloads?: number | null
    followers?: number | null
    filename?: string | null
}

export type ContentModpackCardVersion = Pick<
    Labrinth.Versions.v2.Version,
    'id' | 'version_number' | 'date_published'
>

export type ContentModpackCardCategory = Labrinth.Tags.v2.Category & {
    action?: (event: MouseEvent) => void
}
