import type { Labrinth } from '@modrinth/api-client'
import type { Component } from 'vue'
import type { RouteLocationRaw } from 'vue-router'

export type BrowseDisplayMode = 'list' | 'compact' | 'grid' | 'gallery'

export interface BrowseDisplayModeOption {
    id: BrowseDisplayMode
    label: string
    icon: Component
}

export interface BrowseSearchResponse {
    projectHits: (Labrinth.Search.v2.ResultSearchProject & {
        installed?: boolean
        installing?: boolean
        provider: 'modrinth' | 'curseforge' | 'mcarchive' | 'planet_minecraft'
        providers?: ('modrinth' | 'curseforge' | 'mcarchive' | 'planet_minecraft')[]
        provider_project_id?: string
        author_url?: string
        resource_dates?: {
            createdAt?: string | null
            projectPublishedAt?: string | null
            updatedAt?: string | null
            firstPublishedAt?: string | null
            latestPublishedAt?: string | null
        }
    })[]
    serverHits: Labrinth.Search.v3.ResultSearchProject[]
    resourceNotice?: string
    total_hits: number
    has_more?: boolean
    total_precision?: 'exact' | 'estimate' | 'unknown'
    per_page: number
    page?: number
}

export interface BrowseSelectedProject {
    id: string
    name: string
    iconUrl?: string | null
}

export interface BrowseInstallContext {
    showInstallHeader?: boolean
    name: string
    loader: string
    gameVersion: string
    serverId?: string | null
    upstream?: { project_id?: string | null } | null
    iconSrc?: string | null
    iconFrameless?: boolean
    isMedal?: boolean
    backUrl: string | RouteLocationRaw
    backLabel: string
    heading: string
    warning?: string
    queuedCount?: number
    queuedLabel?: string
    clearQueued?: () => void | Promise<void>
    onBack?: () => boolean | void | Promise<boolean | void>
    selectedProjects?: BrowseSelectedProject[]
    isInstallingSelected?: boolean
    skipNonEssentialWarnings?: boolean
    installProgress?: {
        completed: number
        total: number
    }
    installButtonLabel?: string
    processingLabel?: string
    clearSelected?: () => void | Promise<void>
    discardSelectedAndBack?: () => void | Promise<void>
    installSelected?: () => boolean | void | Promise<boolean | void>
}

export interface CardAction {
    key: string
    label: string
    compactLabel?: string
    icon: Component
    iconClass?: string
    disabled?: boolean
    color?: 'brand' | 'red' | 'green'
    type?: 'standard' | 'outlined' | 'transparent'
    circular?: boolean
    tooltip?: string
    onClick: () => void | Promise<void>
}

export interface ServerModpackContent {
    name: string
    icon?: string
    onclick?: () => void
    showCustomModpackTooltip: boolean
}
