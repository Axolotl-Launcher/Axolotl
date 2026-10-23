import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { injectNotificationManager } from '@modrinth/ui/src/providers/web-notifications.ts'

import { projectIcon } from '../../../../data'
import { usePreviewDomain } from '../../../../lib/state/index'
import type { ContentItem, GameInstance, InstanceLoader } from '../../../../lib/types'
import { messages } from '../../messages'
import { useSettingsPage } from '../../state'
import { canonicalize, childNamed, type FsNode, sortedChildren, useMacFileSystem } from './mac-fs'

/** `ExternalMinecraftRootMode` (helpers/instance.ts). */
export type ExternalMinecraftRootMode = 'automatic' | 'isolated' | 'shared'

/** A configured `.minecraft` root (`ExternalMinecraftRoot`). */
export interface ExternalMinecraftRoot {
    path: string
    mode: ExternalMinecraftRootMode
}

/** `DirectLinkSyncReport`. */
export interface DirectLinkSyncReport {
    imported: number
    updated: number
    removed: number
    missing: number
    errors: string[]
}

interface DirectLinkSyncDomain {
    requestedRoots: ExternalMinecraftRoot[]
    worker: Promise<DirectLinkSyncReport> | null
    pending: boolean
    /** App.vue's `directLinkSyncErrorSignature`. */
    errorSignature: string
}

/**
 * The `.minecraft` root a direct-link record belongs to. Records keep the
 * external version folder as their path and the root as their group
 * (`direct_link_group`), as the import flow creates them too.
 */
export function directLinkRoot(instance: GameInstance): string | null {
    const match = /^(.*)\/versions\/[^/]+$/.exec(instance.path)
    if (!match) return null
    return instance.groups.includes(match[1]) ? match[1] : null
}

function hasVersionManifest(folder: FsNode): FsNode | null {
    return (
        (folder.children ?? []).find(
            (child) =>
                child.kind === 'file' &&
                /\.json$/i.test(child.name) &&
                child.meta?.type === 'version-json',
        ) ?? null
    )
}

/** The mods a direct-link record shows: the shared `mods` folder of its `.minecraft`. */
function contentItemsFor(modsFolder: FsNode | null): ContentItem[] {
    return (modsFolder?.children ?? [])
        .filter((child) => child.kind === 'file' && /\.jar$/i.test(child.name))
        .map((child) => {
            const meta = child.meta?.type === 'mod' ? child.meta : null
            return {
                id: `mods/${child.name}`,
                project_id: meta?.projectId ?? '',
                version_id: meta?.versionId ?? '',
                name: meta?.title ?? child.name.replace(/\.jar$/i, ''),
                file_name: child.name,
                enabled: true,
                icon: meta ? projectIcon(meta.projectId) : null,
                version_number: meta?.versionNumber ?? '',
                project_type: 'mod' as const,
            }
        })
}

/**
 * `syncConfiguredDirectLinks` (helpers/direct-link-sync.ts): serializes
 * reconciliations and keeps the latest roots while one is in flight, then
 * reports problems as App.vue's `handleDirectLinkSyncReport` does.
 */
export function useDirectLinkSync() {
    const { preview, simulate } = useSettingsPage()
    const { addNotification } = injectNotificationManager()
    const { formatMessage } = useVIntl()
    const fs = useMacFileSystem()
    const domain = usePreviewDomain<DirectLinkSyncDomain>('settings:direct-links', () => ({
        value: { requestedRoots: [], worker: null, pending: false, errorSignature: '' },
    }))

    /** `instance_sync_direct_links` (`sync_direct_link_instances.rs`). */
    async function reconcile(roots: ExternalMinecraftRoot[]): Promise<DirectLinkSyncReport> {
        const disk = await fs.load()
        await simulate(180)
        const report: DirectLinkSyncReport = {
            imported: 0,
            updated: 0,
            removed: 0,
            missing: 0,
            errors: [],
        }
        const canonicalRoots: { path: string; node: FsNode; mode: ExternalMinecraftRootMode }[] = []
        for (const root of roots) {
            const resolved = canonicalize(disk, root.path)
            if (!resolved) {
                report.missing += 1
                report.errors.push(
                    `${root.path}: No such file or directory (os error 2), path: ${root.path}`,
                )
                continue
            }
            if (resolved.node.kind !== 'dir') {
                report.missing += 1
                report.errors.push(`${root.path} is not a directory`)
                continue
            }
            if (!canonicalRoots.some((entry) => entry.path === resolved.path)) {
                canonicalRoots.push({ path: resolved.path, node: resolved.node, mode: root.mode })
            }
        }
        canonicalRoots.sort((a, b) => (a.path < b.path ? -1 : a.path > b.path ? 1 : 0))

        const instances = preview.instances
        const seenFolders: string[] = []
        for (const root of canonicalRoots) {
            const versions = childNamed(root.node, 'versions')
            if (!versions || versions.kind !== 'dir') {
                report.errors.push(`${root.path}/versions: No such file or directory (os error 2)`)
                continue
            }
            for (const folder of sortedChildren(versions)) {
                if (folder.kind !== 'dir') continue
                const manifest = hasVersionManifest(folder)
                if (manifest?.meta?.type !== 'version-json') continue
                const folderPath = `${root.path}/versions/${folder.name}`
                seenFolders.push(folderPath)
                const existing = instances.instances.value.find(
                    (instance) => instance.path === folderPath,
                )
                if (existing) {
                    if (!existing.groups.includes(root.path)) {
                        instances.createGroup(root.path)
                        instances.setGroups(existing.id, [root.path])
                        report.updated += 1
                    }
                    continue
                }
                await simulate(40)
                const created = instances.create({
                    name: folder.name,
                    gameVersion: manifest.meta.gameVersion,
                    loader: manifest.meta.loader as InstanceLoader,
                    // Directly associated instances display no loader version.
                    loaderVersion: null,
                    iconPath: null,
                })
                const instance = instances.get(created.id) ?? created
                instance.path = folderPath
                instances.createGroup(root.path)
                instances.setGroups(instance.id, [root.path])
                preview.downloads.contents.value = {
                    ...preview.downloads.contents.value,
                    [instance.id]:
                        manifest.meta.loader === 'vanilla'
                            ? []
                            : contentItemsFor(childNamed(root.node, 'mods')),
                }
                report.imported += 1
            }
        }

        // Configured roots are authoritative: dropping one removes only the
        // launcher's records; the external files stay where they are.
        const touchedGroups = new Set<string>()
        for (const instance of [...instances.instances.value]) {
            const root = directLinkRoot(instance)
            if (!root) continue
            const configured = canonicalRoots.some((entry) => entry.path === root)
            if (
                configured &&
                (seenFolders.includes(instance.path) || canonicalize(disk, instance.path))
            ) {
                continue
            }
            instances.remove(instance.id)
            touchedGroups.add(root)
            report.removed += 1
        }
        for (const group of touchedGroups) {
            if (!instances.instances.value.some((instance) => instance.groups.includes(group))) {
                instances.deleteGroup(group)
            }
        }
        return report
    }

    /** App.vue `handleDirectLinkSyncReport`. */
    function onSynced(report: DirectLinkSyncReport) {
        const details = report.errors.join('\n')
        if (!details) {
            domain.errorSignature = ''
            return
        }
        if (details === domain.errorSignature) return
        domain.errorSignature = details
        addNotification({
            title: formatMessage(messages.appDirectLinkSyncIssuesTitle),
            text: details,
            type: 'warning',
        })
    }

    async function drain(): Promise<DirectLinkSyncReport> {
        let latestReport: DirectLinkSyncReport = {
            imported: 0,
            updated: 0,
            removed: 0,
            missing: 0,
            errors: [],
        }
        try {
            while (domain.pending) {
                domain.pending = false
                latestReport = await reconcile(domain.requestedRoots)
                onSynced(latestReport)
            }
            return latestReport
        } finally {
            domain.worker = null
        }
    }

    function sync(roots: readonly ExternalMinecraftRoot[]): Promise<DirectLinkSyncReport> {
        domain.requestedRoots = roots.map((root) => ({ ...root }))
        domain.pending = true
        domain.worker ??= Promise.resolve().then(drain)
        return domain.worker
    }

    return { sync }
}
