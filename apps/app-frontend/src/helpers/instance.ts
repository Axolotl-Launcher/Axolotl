/**
 * All theseus API calls return serialized values (both return values and errors);
 * So, for example, addDefaultInstance creates a blank instance object, where the Rust struct is serialized,
 *  and deserialized into a usable JS object.
 */
import type { Labrinth } from '@modrinth/api-client'
import {
    clearPinnedContentViewPreferences,
    type ContentItem,
    type ContentOwner,
} from '@modrinth/ui'
import { convertFileSrc, invoke } from '@tauri-apps/api/core'

import { isOfflineMode } from '@/composables/useNetworkStatus'
import { buildGcCandidateChain, resolveAutoGcStrategy } from '@/helpers/gc/auto-selector'
import { collectGcContext } from '@/helpers/gc/context'
import { detectGcStrategy, GC_STRATEGY_DEFINITIONS } from '@/helpers/gc/strategies'
import type { GcContext, ResolvedGcStrategyId } from '@/helpers/gc/types'
import { setLastGcLaunchReport } from '@/helpers/gc-notice'
import { AUTO_GC_PRESET_ARG } from '@/helpers/java-arguments'
import { get_jre, get_memory_status } from '@/helpers/jre.js'
import { get as getSettings } from '@/helpers/settings'

import type { InstallJobSnapshot } from './install'
import { removeInstanceCache } from './instance-cache'
import type {
    CacheBehaviour,
    ContentFile,
    ContentFileProjectType,
    GameInstance,
    InstanceLoader,
} from './types'

export interface InstancePostUpgradeWarning {
    code: import('./instance-upgrade').InstanceUpgradeIssueCode
    contentId: string | null
    relativePath: string | null
}

export interface InstancePostUpgradeNotice {
    instanceId: string
    upgradeJobId: string
    targetGameVersion: string
    consecutiveCleanLaunches: number
    warnings: InstancePostUpgradeWarning[]
}

export async function get_post_upgrade_notice(
    instanceId: string,
): Promise<InstancePostUpgradeNotice | null> {
    return await invoke('plugin:instance|instance_get_post_upgrade_notice', { instanceId })
}

export async function dismiss_post_upgrade_notice(instanceId: string): Promise<void> {
    await invoke('plugin:instance|instance_dismiss_post_upgrade_notice', { instanceId })
}

export async function remove(instanceId: string): Promise<void> {
    await invoke('plugin:instance|instance_remove', { instanceId })
    removeInstanceCache(instanceId)
    clearPinnedContentViewPreferences(instanceId)
}

export interface CreateDirectLinkInstanceRequest {
    name?: string | null
    launcherType: 'HMCL' | 'PCL2' | 'PCL2CE' | 'Generic'
    basePath: string
    instanceFolder: string
    instancePath?: string | null
}

/** Associate an existing `.minecraft/versions/<id>` directory in place. */
export async function create_direct_link(request: CreateDirectLinkInstanceRequest) {
    return await invoke<GameInstance>('plugin:instance|instance_create_direct_link', { request })
}

export interface DirectLinkSyncReport {
    imported: number
    updated: number
    removed: number
    missing: number
    errors: string[]
}

export type ExternalMinecraftRootMode = 'automatic' | 'isolated' | 'shared'

export interface ExternalMinecraftRoot {
    path: string
    mode: ExternalMinecraftRootMode
}

export async function sync_direct_links(
    roots: ExternalMinecraftRoot[],
): Promise<DirectLinkSyncReport> {
    return await invoke<DirectLinkSyncReport>('plugin:instance|instance_sync_direct_links', {
        roots,
    })
}

export async function get(instanceId: string): Promise<GameInstance | null> {
    return await invoke('plugin:instance|instance_get', { instanceId })
}

export async function get_many(instanceIds: string[]): Promise<GameInstance[]> {
    return await invoke('plugin:instance|instance_get_many', { instanceIds })
}

export type SyncedOption =
    | 'game_options'
    | 'command_history'
    | 'multiplayer_servers'
    | 'creative_hotbars'
    | 'screenshots'
    | 'resource_packs'
    | 'data_packs'
export type GlobalSyncedOptions = Record<SyncedOption, boolean>
export type SyncedOptionCapability = {
    option: SyncedOption
    supported: boolean
    disabled_reason: string | null
}
export type SyncedOptionJoinPreview = { action: string }
export type SyncedOptionsOverview = {
    global_options: GlobalSyncedOptions
    capabilities: SyncedOptionCapability[]
}
export async function get_global_synced_options(): Promise<GlobalSyncedOptions> {
    return await invoke('plugin:instance|instance_get_synced_options')
}
export async function get_initialized_synced_options(): Promise<GlobalSyncedOptions> {
    return await invoke('plugin:instance|instance_get_initialized_synced_options')
}
/**
 * Every synced option is currently available on every instance. The parameter
 * stays so callers keep passing the option they are asking about, which is what
 * a real implementation will need.
 */
export function isSyncedOptionAvailable(_option: SyncedOption): boolean {
    return true
}
export async function set_global_synced_option(
    option: SyncedOption,
    enabled: boolean,
    baseInstanceId?: string | null,
): Promise<GlobalSyncedOptions> {
    return await invoke('plugin:instance|instance_set_synced_option', {
        option,
        enabled,
        baseInstanceId,
    })
}
export async function set_instance_synced_option(
    instanceId: string,
    option: SyncedOption,
    enabled: boolean,
    resolution?: string | null,
): Promise<GameInstance> {
    return await invoke('plugin:instance|instance_set_instance_synced_option', {
        instanceId,
        option,
        enabled,
        resolution,
    })
}
export async function get_synced_options_overview(
    instanceId: string,
): Promise<SyncedOptionsOverview> {
    return await invoke('plugin:instance|instance_get_synced_options_overview', { instanceId })
}
export async function get_command_history(): Promise<string> {
    return await invoke('plugin:instance|instance_get_synced_command_history')
}
export async function set_command_history(contents: string): Promise<string> {
    return await invoke('plugin:instance|instance_set_synced_command_history', { contents })
}
export type SyncedServer = {
    id: string
    name: string
    address: string
    accept_textures: boolean | null
}
export async function list_synced_servers(): Promise<SyncedServer[]> {
    return await invoke('plugin:instance|instance_list_synced_servers')
}
export async function update_synced_server(server: SyncedServer): Promise<void> {
    await invoke('plugin:instance|instance_update_synced_server', { server })
}
export async function remove_synced_server(id: string): Promise<void> {
    await invoke('plugin:instance|instance_remove_synced_server', { id })
}

export async function get_projects(
    instanceId: string,
    cacheBehaviour?: CacheBehaviour,
): Promise<Record<string, ContentFile>> {
    return await invoke('plugin:instance|instance_get_projects', { instanceId, cacheBehaviour })
}

export async function get_installed_project_ids(instanceId: string): Promise<string[]> {
    return await invoke('plugin:instance|instance_get_installed_project_ids', { instanceId })
}

export type CoreComponentKind = 'jar_mod' | 'replacement_jar' | 'agent'

export type CoreComponentSource = {
    provider: string
    projectId: string | null
    versionId: string | null
    fileId: string | null
    pageUrl: string | null
}

export type CoreComponent = {
    id: string
    kind: CoreComponentKind
    fileName: string
    relativePath: string
    enabled: boolean
    removed: boolean
    order: number
    sha1: string | null
    sha256: string | null
    source: CoreComponentSource | null
    targetGameVersion: string
    createdAt: string
    modifiedAt: string
    failureReason: string | null
}

export type CoreJarPreview = {
    outputPath: string
    componentCount: number
    replacementComponentId: string | null
    entries: number
    sha1: string
    sha256: string
}

export type McArchiveCoreInstallResult =
    | { state: 'installed'; component: CoreComponent }
    | {
          state: 'manual_download'
          fileName: string
          pageUrl: string | null
          expectedSha256: string | null
      }

export async function list_core_components(instanceId: string): Promise<CoreComponent[]> {
    return await invoke('plugin:instance|instance_list_core_components', { instanceId })
}

export async function add_core_jar_mod(
    instanceId: string,
    sourcePath: string,
    targetGameVersion: string,
    source?: CoreComponentSource,
): Promise<CoreComponent> {
    return await invoke('plugin:instance|instance_add_core_jar_mod', {
        instanceId,
        sourcePath,
        targetGameVersion,
        source,
    })
}

export async function replace_core_jar(
    instanceId: string,
    sourcePath: string,
    targetGameVersion: string,
    source?: CoreComponentSource,
): Promise<CoreComponent> {
    return await invoke('plugin:instance|instance_replace_core_jar', {
        instanceId,
        sourcePath,
        targetGameVersion,
        source,
    })
}

export async function move_core_component(
    instanceId: string,
    componentId: string,
    direction: -1 | 1,
): Promise<CoreComponent[]> {
    return await invoke('plugin:instance|instance_move_core_component', {
        instanceId,
        componentId,
        direction,
    })
}

export async function set_core_component_enabled(
    instanceId: string,
    componentId: string,
    enabled: boolean,
): Promise<CoreComponent> {
    return await invoke('plugin:instance|instance_set_core_component_enabled', {
        instanceId,
        componentId,
        enabled,
    })
}

export async function remove_core_component(
    instanceId: string,
    componentId: string,
): Promise<void> {
    await invoke('plugin:instance|instance_remove_core_component', { instanceId, componentId })
}

export async function restore_core_component(
    instanceId: string,
    componentId: string,
): Promise<CoreComponent> {
    return await invoke('plugin:instance|instance_restore_core_component', {
        instanceId,
        componentId,
    })
}

export async function preview_core_jar(instanceId: string): Promise<CoreJarPreview | null> {
    return await invoke('plugin:instance|instance_preview_core_jar', { instanceId })
}

export async function install_mcarchive_modloader(
    instanceId: string,
    gameVersion: string,
): Promise<McArchiveCoreInstallResult> {
    return await invoke('plugin:instance|instance_install_mcarchive_modloader', {
        instanceId,
        gameVersion,
    })
}

export async function import_mcarchive_modloader(
    instanceId: string,
    gameVersion: string,
    sourcePath: string,
): Promise<McArchiveCoreInstallResult> {
    return await invoke('plugin:instance|instance_import_mcarchive_modloader', {
        instanceId,
        gameVersion,
        sourcePath,
    })
}

export type InstanceInstallTarget = {
    game_version: string
    loader: string
}

export type InstanceInstallCandidate = {
    id: string
    name: string
    icon_path?: string | null
    game_version: string
    loader: InstanceLoader
    installed: boolean
    compatible: boolean
}

export async function get_install_candidates(
    projectId: string,
    projectType: string,
    targets: InstanceInstallTarget[],
): Promise<InstanceInstallCandidate[]> {
    return await invoke('plugin:instance|instance_get_install_candidates', {
        projectId,
        projectType,
        targets,
    })
}

// Get content items with rich metadata for an instance
// Returns content items filtered to exclude modpack files (if linked),
// sorted alphabetically by project name
export async function get_content_items(
    instanceId: string,
    cacheBehaviour?: CacheBehaviour,
): Promise<ContentItem[]> {
    const items = await invoke<ContentItem[]>('plugin:instance|instance_get_content_items', {
        instanceId,
        cacheBehaviour,
    })
    return items
}

export async function get_content_items_by_paths(
    instanceId: string,
    paths: string[],
    cacheBehaviour?: CacheBehaviour,
): Promise<ContentItem[]> {
    return await invoke<ContentItem[]>('plugin:instance|instance_get_content_items_by_paths', {
        instanceId,
        paths,
        cacheBehaviour,
    })
}

export type ContentOwnershipKind = 'pack_managed' | 'user_added' | 'local_discovered'
export type PackMemberMaterializationState = 'present' | 'pending_manual' | 'missing' | 'removed'
export type PackMemberOverrideKind = 'none' | 'disabled' | 'removed' | 'version'

export interface InstanceContentSnapshotItem {
    artifactRef?: import('./resources').ResourceArtifact['reference'] | null
    resourceId?: string | null
    packResourceId?: string | null
    compatibility?: ContentItem['compatibility']
    fileId: string | null
    entryId: string | null
    memberId: string | null
    ownershipKind: ContentOwnershipKind
    materializationState: PackMemberMaterializationState
    overrideKind: PackMemberOverrideKind
    expectedRelativePath: string
    required: boolean
    projectType: string
    provider: 'modrinth' | 'curseforge' | 'mcarchive' | null
    providerProjectId: string | null
    providerReleaseId: string | null
    content: ContentItem | null
    capabilities: {
        canToggle: boolean
        canDelete: boolean
        canUpdate: boolean
        canChangeVersion: boolean
        canRestorePackDefault: boolean
    }
    dependency: {
        autoDependency: boolean
        requiredBy: Array<{
            provider: 'modrinth' | 'curseforge' | 'mcarchive' | 'local'
            projectId: string
            releaseId: string
        }>
        requires: Array<{
            provider: 'modrinth' | 'curseforge' | 'mcarchive' | 'local'
            projectId: string
            releaseId: string
        }>
        orphaned: boolean
    } | null
}

export interface PendingManualDownload {
    id: string
    instanceId: string
    packMemberId: string | null
    contentEntryId: string | null
    operationKind: 'pack_install' | 'pack_update' | 'content_install' | 'content_update'
    operationTargetId: string | null
    projectType: string
    provider: 'modrinth' | 'curseforge' | 'mcarchive'
    providerProjectId: string
    providerReleaseId: string
    fileName: string
    websiteUrl: string | null
    targetRelativePath: string
    expectedSha1: string | null
    expectedSize: number | null
    expectedFingerprint: number | null
    state: 'waiting' | 'matched' | 'imported' | 'error' | 'cancelled'
    context: Record<string, unknown>
    createdAt: string
    modifiedAt: string
}

export interface InstanceContentSnapshot {
    instanceId: string
    revision: number
    pack: {
        name: string
        iconPath: string | null
        provider: 'modrinth' | 'curseforge' | 'mcarchive' | null
        projectId: string | null
        versionId: string | null
        reconciled: boolean
        canUpdate: boolean
        metadata: LinkedModpackInfo | null
    } | null
    items: InstanceContentSnapshotItem[]
    pendingManualDownloads: PendingManualDownload[]
    warnings: Array<{
        code: string
        message: string
        provider: 'modrinth' | 'curseforge' | 'mcarchive' | null
    }>
}

export async function get_content_snapshot(
    instanceId: string,
    deferEnrichment = false,
): Promise<InstanceContentSnapshot> {
    return await invoke('plugin:instance|instance_get_content_snapshot', {
        instanceId,
        deferEnrichment,
    })
}

export async function refresh_content(instanceId: string): Promise<InstanceContentSnapshot> {
    return await invoke('plugin:instance|instance_refresh_content', { instanceId })
}

export type ContentUpdateScope = 'user_added' | 'pack' | 'item'

export interface ContentUpdatePlan {
    id: string
    instanceId: string
    revision: number
    scope: ContentUpdateScope
    actions: Array<{
        contentId: string
        relativePath: string | null
        ownershipKind: ContentOwnershipKind
        provider: 'modrinth' | 'curseforge' | 'mcarchive'
        currentReleaseId: string | null
        targetReleaseId: string
    }>
    warnings: string[]
}

export interface ContentUpdateResolution {
    contentId: string
    choice: 'keep_override' | 'restore_pack_default'
}

export async function plan_content_updates(
    instanceId: string,
    scope: ContentUpdateScope,
    target?: string,
): Promise<ContentUpdatePlan> {
    return await invoke('plugin:instance|instance_plan_content_updates', {
        instanceId,
        scope,
        target,
    })
}

export async function apply_content_update_plan(
    planId: string,
    resolutions: ContentUpdateResolution[] = [],
): Promise<InstanceContentSnapshot> {
    return await invoke('plugin:instance|instance_apply_content_update_plan', {
        planId,
        resolutions,
    })
}

// Linked modpack info returned from backend
export interface LinkedModpackInfo {
    project: Labrinth.Projects.v2.Project
    version: Labrinth.Versions.v2.Version
    owner: ContentOwner | null
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
    update_version: Labrinth.Versions.v2.Version | null
}

// Get linked modpack info for an instance
// Returns project, version, and owner information for the linked modpack,
// or null if the instance is not linked to a modpack
export async function get_linked_modpack_info(
    instanceId: string,
    cacheBehaviour?: CacheBehaviour,
): Promise<LinkedModpackInfo | null> {
    return await invoke('plugin:instance|instance_get_linked_modpack_info', {
        instanceId,
        cacheBehaviour,
    })
}

// Get content items that are part of the linked modpack
// Returns the modpack's dependencies as ContentItem list
// Returns empty array if the instance is not linked to a modpack
export async function get_linked_modpack_content(
    instanceId: string,
    cacheBehaviour?: CacheBehaviour,
): Promise<ContentItem[]> {
    const items = await invoke<ContentItem[]>(
        'plugin:instance|instance_get_linked_modpack_content',
        {
            instanceId,
            cacheBehaviour,
        },
    )
    return items
}

// Convert a list of dependencies into ContentItems with rich metadata
export async function get_dependencies_as_content_items(
    dependencies: Labrinth.Versions.v3.Dependency[],
    cacheBehaviour?: CacheBehaviour,
): Promise<ContentItem[]> {
    const items = await invoke<ContentItem[]>(
        'plugin:instance|instance_get_dependencies_as_content_items',
        {
            dependencies,
            cacheBehaviour,
        },
    )
    return items
}

export function adaptContentItems(items: ContentItem[]): ContentItem[] {
    return items.map((item) => {
        const embeddedMetadata = item.embedded_metadata
        if (!embeddedMetadata?.icon_path) return item
        return {
            ...item,
            embedded_metadata: {
                ...embeddedMetadata,
                icon_url: convertFileSrc(embeddedMetadata.icon_path),
            },
        }
    })
}

export async function get_full_path(instanceId: string): Promise<string> {
    return await invoke('plugin:instance|instance_get_full_path', { instanceId })
}

export async function get_mod_full_path(instanceId: string, projectPath: string): Promise<string> {
    return await invoke('plugin:instance|instance_get_mod_full_path', { instanceId, projectPath })
}

export interface JavaVersion {
    parsed_version: number
    version: string
    architecture: string
    path: string
}

export async function get_optimal_jre_key(instanceId: string): Promise<JavaVersion | null> {
    return await invoke('plugin:instance|instance_get_optimal_jre_key', { instanceId })
}

export async function list(): Promise<GameInstance[]> {
    return await invoke('plugin:instance|instance_list')
}

export type DailyPlaytime = {
    date: string
    played_seconds: number
    session_count: number
    top_instance_name?: string | null
}

export async function set_pinned(instanceId: string, pinned: boolean): Promise<void> {
    return await invoke('plugin:instance|instance_set_pinned', { instanceId, pinned })
}

export async function get_daily_playtime(
    startDate: string,
    endDate: string,
): Promise<DailyPlaytime[]> {
    return await invoke('plugin:instance|instance_get_daily_playtime', { startDate, endDate })
}

export type DailyPlaytimeEntry = {
    instance_id: string
    instance_name: string
    played_seconds: number
    session_count: number
}

export async function get_daily_playtime_details(date: string): Promise<DailyPlaytimeEntry[]> {
    return await invoke('plugin:instance|instance_get_daily_playtime_details', { date })
}

export async function check_installed(instanceId: string, projectId: string): Promise<boolean> {
    return await invoke('plugin:instance|instance_check_installed', { instanceId, projectId })
}

export type InstallContentBatchItem = { type: 'curse_forge_world'; request: unknown }

export async function queue_content_batch(
    instanceId: string,
    items: InstallContentBatchItem[],
    display: { title: string; iconUrl?: string | null },
): Promise<InstallJobSnapshot> {
    return await invoke('plugin:instance|instance_queue_content_batch', {
        request: {
            instanceId,
            items,
            displayTitle: display.title,
            displayIcon: display.iconUrl ?? null,
        },
    })
}

// Import a world save into an instance
// Returns the imported world name
export async function import_world_save(
    instanceId: string,
    sourcePath: string,
    innerBase?: string,
): Promise<string> {
    return await invoke('plugin:instance|instance_import_world_save', {
        instanceId,
        sourcePath,
        innerBase,
    })
}

export async function install_datapack_to_world(
    instanceId: string,
    worldPath: string,
    sourcePath: string,
    innerBase?: string,
): Promise<string> {
    return await invoke('plugin:instance|instance_install_datapack_to_world', {
        instanceId,
        worldPath,
        sourcePath,
        innerBase,
    })
}

// Add a project to an instance from a path + project_type
// Returns a path to the new project file
export async function add_project_from_path(
    instanceId: string,
    projectPath: string,
    projectType?: ContentFileProjectType,
    innerBase?: string,
): Promise<string> {
    return await invoke('plugin:instance|instance_add_project_from_path', {
        instanceId,
        projectPath,
        projectType,
        innerBase,
    })
}

// Toggle disabling a project
export async function toggle_disable_project(
    instanceId: string,
    projectPath: string,
    desiredEnabled?: boolean,
): Promise<string> {
    return await invoke('plugin:instance|instance_toggle_disable_project', {
        instanceId,
        projectPath,
        desiredEnabled,
    })
}

export async function toggle_content_entry(
    instanceId: string,
    contentId: string,
    desiredEnabled?: boolean,
): Promise<string> {
    return await invoke('plugin:instance|instance_toggle_content_entry', {
        instanceId,
        contentId,
        desiredEnabled,
    })
}

export type ContentToggleResult = {
    contentId: string
    path: string
    enabled: boolean
}

export async function toggle_content_entries(
    instanceId: string,
    contentIds: string[],
    desiredEnabled?: boolean,
): Promise<ContentToggleResult[]> {
    return await invoke('plugin:instance|instance_toggle_content_entries', {
        instanceId,
        contentIds,
        desiredEnabled,
    })
}

// Roll back an updated project to its previous file (kept as a .old backup)
export async function rollback_project(instanceId: string, projectPath: string): Promise<string> {
    return await invoke('plugin:instance|instance_rollback_project', {
        instanceId,
        projectPath,
    })
}

// Remove a project
export async function remove_project(instanceId: string, projectPath: string): Promise<void> {
    return await invoke('plugin:instance|instance_remove_project', { instanceId, projectPath })
}

export async function remove_content_entry(instanceId: string, contentId: string): Promise<void> {
    return await invoke('plugin:instance|instance_remove_content_entry', { instanceId, contentId })
}

export type ContentChangeIntent =
    | { type: 'update_one'; content_id: string; target_release_id?: string | null }
    | {
          type: 'update_selected'
          targets: Array<{ content_id: string; target_release_id: string }>
      }
    | { type: 'update_all_user_added' }
    | { type: 'switch_version'; content_id: string; target_release_id: string }

export async function queue_content_change(
    instanceId: string,
    intent: ContentChangeIntent,
    displayTitle: string,
    displayIcon?: string,
): Promise<InstallJobSnapshot> {
    return await invoke('plugin:instance|instance_queue_content_change', {
        instanceId,
        intent,
        displayTitle,
        displayIcon,
    })
}

export async function restore_pack_member_default(
    instanceId: string,
    memberId: string,
): Promise<string | null> {
    return await invoke('plugin:instance|instance_restore_pack_member_default', {
        instanceId,
        memberId,
    })
}

// Update a managed Modrinth instance to a specific version
export async function update_managed_modrinth_version(
    instanceId: string,
    versionId: string,
): Promise<InstallJobSnapshot> {
    return await invoke('plugin:instance|instance_update_managed_modrinth_version', {
        instanceId,
        versionId,
    })
}

// Repair a managed Modrinth instance
export async function update_repair_modrinth(instanceId: string): Promise<InstallJobSnapshot> {
    return await invoke('plugin:instance|instance_repair_managed_modrinth', { instanceId })
}

export interface GcLaunchIntent {
    active_preset_id: string
    block_tokens: string[]
    candidate_ids: string[]
    candidates: string[][]
}

export interface GcLaunchReport {
    preferred_strategy: string
    chosen_strategy: string
    chosen_args: string[]
    pruned_args: string[]
    reason_chain: string[]
}

export interface InstanceRunResult {
    process: unknown
    gc_notice: GcLaunchReport | null
}

export function gcReportFellBack(report: GcLaunchReport): boolean {
    return report.chosen_strategy !== report.preferred_strategy || report.pruned_args.length > 0
}

/**
 * The Java that will actually run the instance: a per-instance `java_path`
 * wins, otherwise the optimal JRE for the game version.
 */
async function resolveEffectiveJava(
    instance: Pick<GameInstance, 'java_path'>,
    instanceId: string,
): Promise<JavaVersion | null> {
    if (instance.java_path) {
        try {
            return await get_jre(instance.java_path)
        } catch {
            // Invalid override; fall back to the optimal JRE.
        }
    }
    return await get_optimal_jre_key(instanceId)
}

function normalizeJvmToken(token: string): string {
    const stripped = token.replace(/^-+/, '')
    const withoutPrefix = stripped.startsWith('XX:') ? stripped.slice(3) : stripped
    const withoutModifier =
        withoutPrefix.startsWith('+') || withoutPrefix.startsWith('-')
            ? withoutPrefix.slice(1)
            : withoutPrefix
    return withoutModifier.split('=')[0] ?? withoutModifier
}

function strategyTokenKeys(strategy: ResolvedGcStrategyId, context: GcContext): Set<string> {
    const tokens = GC_STRATEGY_DEFINITIONS[strategy].buildArgs(context).split(/\s+/).filter(Boolean)
    return new Set(tokens.map(normalizeJvmToken))
}

/**
 * Determine whether the effective Java args contain an auto GC marker or an
 * active GC preset block, and if so build the ordered candidate chain for the
 * backend to verify against the actual JVM. The args are returned untouched so
 * the backend replaces exactly the preset block.
 */
export async function resolveGcLaunchIntent(
    instanceId: string,
): Promise<{ args: string[]; gcIntent: GcLaunchIntent | null }> {
    const instance = await get(instanceId)
    if (!instance) return { args: [], gcIntent: null }

    const settings = await getSettings()
    const effectiveArgs = instance.extra_launch_args ?? settings.extra_launch_args

    const java = await resolveEffectiveJava(instance, instanceId)
    const javaMajorVersion = java?.parsed_version ?? null
    let modCount = 0
    try {
        const snapshot = await get_content_snapshot(instanceId)
        modCount = snapshot.items.filter(
            (item) => item.projectType === 'mod' && item.materializationState === 'present',
        ).length
    } catch {
        modCount = 0
    }

    const memory = instance.memory ?? settings.memory
    let allocatedMemoryMb = memory.maximum
    if (memory.automatic) {
        try {
            const status = await get_memory_status(instanceId, memory.maximum, true)
            allocatedMemoryMb = status.allocated_mb
        } catch {
            // Fall back to the configured value when automatic memory info is unavailable.
        }
    }
    const context = await collectGcContext(
        allocatedMemoryMb,
        instance.loader,
        javaMajorVersion,
        modCount,
    )

    let activePresetId: string | null = null
    let blockTokens: string[] = []
    if (effectiveArgs.includes(AUTO_GC_PRESET_ARG)) {
        activePresetId = 'gc-auto'
        blockTokens = [AUTO_GC_PRESET_ARG]
    } else {
        const detected = detectGcStrategy(effectiveArgs.join(' '))
        if (detected) {
            activePresetId = `gc-${detected}`
            const presetKeys = strategyTokenKeys(detected, context)
            blockTokens = effectiveArgs.filter((arg) => presetKeys.has(normalizeJvmToken(arg)))
        }
    }

    if (!activePresetId || !blockTokens.length) {
        return { args: effectiveArgs, gcIntent: null }
    }

    const preferred: ResolvedGcStrategyId =
        activePresetId === 'gc-auto'
            ? resolveAutoGcStrategy(context).resolvedStrategy
            : (activePresetId.slice('gc-'.length) as ResolvedGcStrategyId)

    const { ids, args: candidates } = buildGcCandidateChain(context, preferred)

    return {
        args: effectiveArgs,
        gcIntent: {
            active_preset_id: activePresetId,
            block_tokens: blockTokens,
            candidate_ids: ids,
            candidates,
        },
    }
}

// Run Minecraft using an instance
// Returns the process metadata plus an optional GC fallback report.
export async function run(
    instanceId: string,
    serverAddress: string | null = null,
): Promise<InstanceRunResult> {
    const { args, gcIntent } = await resolveGcLaunchIntent(instanceId)
    const result = await invoke<InstanceRunResult>('plugin:instance|instance_run', {
        instanceId,
        serverAddress,
        offlineMode: isOfflineMode(),
        extraLaunchArgs: args,
        gcIntent,
    })
    setLastGcLaunchReport(result.gc_notice)
    return result
}

export async function kill(instanceId: string): Promise<void> {
    return await invoke('plugin:instance|instance_kill', { instanceId })
}

// Edits an instance
export async function edit(instanceId: string, editInstance: Partial<GameInstance>): Promise<void> {
    return await invoke('plugin:instance|instance_edit', { instanceId, editInstance })
}

export async function cache_icon(iconName: string, bytes: number[]): Promise<string> {
    return await invoke('plugin:instance|instance_cache_icon', { iconName, bytes })
}

// Edits an instance's icon
export async function edit_icon(instanceId: string, iconPath: string | null): Promise<void> {
    return await invoke('plugin:instance|instance_edit_icon', { instanceId, iconPath })
}

export type SymlinkCapability = 'supported' | 'requires_admin' | 'unsupported'

export async function check_symlink_capability(): Promise<SymlinkCapability> {
    return await invoke('check_symlink_capability')
}

export async function allow_symlink_target(path: string): Promise<void> {
    return await invoke('allow_symlink_target', { path })
}

export type ScreenshotKey = { instance_id: string; file_name: string }
export type InstanceScreenshot = {
    id: string
    instance_id: string
    instance_name: string
    file_name: string
    created_at: string
    modified_at: number
    group_id: string | null
    path: string
    url: string
}
export type ScreenshotGroup = { id: string; name: string }
export type ScreenshotScanResult = {
    screenshots: InstanceScreenshot[]
    skipped_instances: string[]
}
export type ScreenshotGroupImport = ScreenshotGroup & { screenshot_ids: string[] }
export type ScreenshotGroupMembershipUpdate = { screenshot_id: string; group_id: string | null }

export async function list_screenshots(instanceId: string): Promise<InstanceScreenshot[]> {
    return await invoke('plugin:instance|instance_list_screenshots', { instanceId })
}
export async function list_all_screenshots(): Promise<InstanceScreenshot[]> {
    return (await invoke<ScreenshotScanResult>('plugin:instance|instance_list_all_screenshots'))
        .screenshots
}
export async function list_synced_screenshots(): Promise<InstanceScreenshot[]> {
    return (await list_synced_screenshots_with_warnings()).screenshots
}
export async function list_synced_screenshots_with_warnings(): Promise<ScreenshotScanResult> {
    return await invoke('plugin:instance|instance_list_synced_screenshots')
}
export async function list_screenshot_groups(): Promise<ScreenshotGroup[]> {
    return await invoke('plugin:instance|instance_list_screenshot_groups')
}
export async function create_screenshot_group(
    name: string,
    screenshotIds: string[],
): Promise<ScreenshotGroup> {
    return await invoke('plugin:instance|instance_create_screenshot_group', {
        name,
        ids: screenshotIds,
    })
}
export async function rename_screenshot_group(
    id: string,
    newName: string,
): Promise<ScreenshotGroup> {
    return await invoke('plugin:instance|instance_rename_screenshot_group', { id, newName })
}
export async function delete_screenshot_group(id: string): Promise<void> {
    return await invoke('plugin:instance|instance_delete_screenshot_group', { id })
}
export async function set_screenshot_group_memberships(
    updates: ScreenshotGroupMembershipUpdate[],
): Promise<void> {
    return await invoke('plugin:instance|instance_set_screenshot_group_memberships', { updates })
}
export async function import_screenshot_groups(groups: ScreenshotGroupImport[]): Promise<void> {
    return await invoke('plugin:instance|instance_import_screenshot_groups', { groups })
}
export async function save_edited_screenshot(
    key: ScreenshotKey,
    pngBytes: Uint8Array,
    mode: 'create_copy' | 'replace_edit',
): Promise<InstanceScreenshot> {
    return await invoke('plugin:instance|instance_save_edited_screenshot', {
        key,
        bytes: Array.from(pngBytes),
        mode,
    })
}
export async function delete_screenshots(keys: ScreenshotKey[]): Promise<void> {
    return await invoke('plugin:instance|instance_delete_screenshots', { keys })
}
export async function export_screenshots(keys: ScreenshotKey[], path: string): Promise<void> {
    return await invoke('plugin:instance|instance_export_screenshots', { keys, path })
}
export function getInstanceIconUrl(iconPath?: string | null): string | undefined {
    return iconPath ? convertFileSrc(iconPath) : undefined
}
export async function open_screenshot(key: ScreenshotKey): Promise<void> {
    return await invoke('plugin:instance|instance_open_screenshot', { key })
}
export async function move_screenshots(
    keys: ScreenshotKey[],
    targetInstanceId: string,
): Promise<ScreenshotKey[]> {
    return await invoke('plugin:instance|instance_move_screenshots', {
        keys,
        target: targetInstanceId,
    })
}
