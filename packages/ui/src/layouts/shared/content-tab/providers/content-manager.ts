import type { ComputedRef, Ref } from 'vue'
import type { RouteLocationRaw } from 'vue-router'

import type { Option as OverflowMenuOption } from '#ui/components/base/OverflowMenu.vue'
import { createContext } from '#ui/providers/create-context'

import type {
    ContentCardTableItem,
    ContentItem,
    ContentModpackCardCategory,
    ContentModpackCardProject,
    ContentModpackCardVersion,
    ContentOwner,
} from '../types'

export interface ContentModpackData {
    project: ContentModpackCardProject
    projectLink?: string | RouteLocationRaw
    version?: ContentModpackCardVersion
    versionLink?: string | RouteLocationRaw
    owner?: ContentOwner
    categories: ContentModpackCardCategory[]
    hasUpdate: boolean
    disabled?: boolean
    disabledText?: string
}

export interface ContentDependencyWarning {
    items: ContentItem[]
    dependents: Array<{
        item: ContentItem
        dependencies: ContentItem[]
    }>
}

export interface ContentManagerContext {
    // Data
    items: Ref<ContentItem[]> | ComputedRef<ContentItem[]>
    /** Items detected as duplicate content; rendered in a dedicated group when present. */
    duplicateItems?: Ref<ContentItem[]> | ComputedRef<ContentItem[]>
    updateDuplicateGroup?: (item: ContentItem) => void | Promise<void>
    loading: Ref<boolean>
    error: Ref<Error | null>
    filterOptionsReady?: Ref<boolean> | ComputedRef<boolean>

    // Modpack
    modpack: Ref<ContentModpackData | null> | ComputedRef<ContentModpackData | null>
    modpackItems?: Ref<ContentItem[]> | ComputedRef<ContentItem[]>

    // Guards
    isBusy: Ref<boolean> | ComputedRef<boolean>
    busyMessage?: Ref<string | null> | ComputedRef<string | null>
    skipNonEssentialWarnings?: Ref<boolean> | ComputedRef<boolean>
    disableAddContent?: Ref<boolean> | ComputedRef<boolean>
    disableAddContentTooltip?: string

    // Labelling
    contentTypeLabel: Ref<string> | ComputedRef<string>

    // Core actions
    toggleEnabled: (item: ContentItem) => Promise<void>
    deleteItem: (item: ContentItem) => Promise<void>
    refresh: () => Promise<void>
    browse: () => void
    uploadFiles: () => void

    // Bulk actions (optional — when provided, used instead of one-by-one loops)
    bulkDeleteItems?: (items: ContentItem[]) => Promise<void>
    bulkEnableItems?: (items: ContentItem[]) => Promise<void | boolean>
    bulkDisableItems?: (items: ContentItem[]) => Promise<void | boolean>
    getDeleteDependencyWarning?: (
        items: ContentItem[],
    ) => ContentDependencyWarning | null | Promise<ContentDependencyWarning | null>

    // Update support (optional per-platform)
    hasUpdateSupport: boolean
    /** Submit an update for the exact row snapshot that produced the action. */
    updateItem?: (item: ContentItem) => void | Promise<void>
    rollbackItem?: (item: ContentItem) => Promise<void>
    /** Submit a platform-owned update-all operation. Resolves once the operation is observable. */
    bulkUpdateAll?: () => Promise<void | boolean>
    bulkUpdateAllLabel?: string
    bulkUpdateAllDescription?: string
    bulkUpdateIncludesModpack?: boolean
    bulkUpdateItem?: (item: ContentItem) => Promise<void>
    bulkUpdateItems?: (items: ContentItem[]) => Promise<void | boolean>

    // Modpack actions (optional)
    updateModpack?: () => void
    viewDependencies?: () => void
    unlinkModpack?: () => void | Promise<void>
    openSettings?: () => void

    // Switch version (optional)
    switchVersion?: (item: ContentItem) => void

    // Per-item overflow menu (optional)
    getOverflowOptions?: (item: ContentItem) => OverflowMenuOption[]

    // Share support (optional — when undefined, share button becomes hidden entirely)
    shareItems?: (
        items: ContentItem[],
        format: 'names' | 'file-names' | 'urls' | 'markdown',
    ) => void

    // Stable per-row identity. ContentItem.id can be a content hash, so it is not always unique.
    getItemId?: (item: ContentItem) => string

    // Bulk operation guard — set by layout, checked by providers to suppress refreshes
    isBulkOperating?: Ref<boolean>

    // Deletion context (controls modal variant)
    deletionContext?: 'instance' | 'server'

    // One-time content hint (optional — shows tooltip on modpack content button)
    showContentHint?: Ref<boolean>
    dismissContentHint?: () => void

    // Symlink target (optional — when set, modals show symlink warnings)
    symlinkTarget?: Ref<string | null | undefined> | ComputedRef<string | null | undefined>

    // Table item mapping (link generation differs per platform)
    mapToTableItem: (item: ContentItem) => ContentCardTableItem

    // 实例/服务器标识，用于内存级 UI 状态隔离（筛选偏好、分组展开等）
    instanceId?: string
}

export const [injectContentManager, provideContentManager] = createContext<ContentManagerContext>(
    'ContentPageLayout',
    'contentManagerContext',
)
