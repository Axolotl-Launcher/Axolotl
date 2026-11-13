export interface CurseForgeManualDownloadItem {
    projectId: number
    fileId: number
    fileName: string
    websiteUrl?: string
    projectType?: string
    projectSlug?: string
    targetFolder?: string
    hashes?: Array<{ value: string; algo: number }>
    fileLength?: number
    fileFingerprint?: number
    ownershipKind?: 'pack_managed' | 'user_added'
    operationKind?: 'pack_install' | 'pack_update' | 'content_install' | 'content_update'
}

export interface InstalledCurseForgeContentItem {
    file_name: string
    provider_refs?: Array<
        | { provider: 'modrinth'; project_id: string; version_id?: string | null }
        | { provider: 'curseforge'; project_id: number; file_id?: number | null }
    >
}

const manualDownloadsByInstance = new Map<string, CurseForgeManualDownloadItem[]>()

function modFileFamily(fileName: string) {
    const baseName = fileName.replace(/\.disabled$/i, '')
    const extension = baseName.match(/\.([^.]+)$/)?.[1]?.toLowerCase()
    if (!extension) return undefined

    const stem = baseName
        .toLowerCase()
        .replace(/\.(?:jar|zip|litemod|mrpack)$/i, '')
        .replace(/\s*\(\d+\)$/, '')
    const versionStart = stem.search(/[-_. ]+v?\d/)
    if (versionStart <= 0) return undefined

    const family = stem.slice(0, versionStart).replace(/[^a-z0-9]+/g, '')
    return family.length >= 3 ? `${extension}:${family}` : undefined
}

export function getCurseForgeManualDownloads(instanceId: string): CurseForgeManualDownloadItem[] {
    return manualDownloadsByInstance.get(instanceId) ?? []
}

export function setCurseForgeManualDownloads(
    instanceId: string,
    items: CurseForgeManualDownloadItem[],
) {
    if (!items.length) {
        manualDownloadsByInstance.delete(instanceId)
        return
    }

    const existing = new Map(
        (manualDownloadsByInstance.get(instanceId) ?? []).map((item) => [
            `${item.projectId}:${item.fileId}`,
            item,
        ]),
    )
    const deduped = new Map<string, CurseForgeManualDownloadItem>()
    for (const item of items) {
        const key = `${item.projectId}:${item.fileId}`
        const previous = existing.get(key)
        deduped.set(key, {
            ...previous,
            ...item,
            projectType: item.projectType ?? previous?.projectType,
            projectSlug: item.projectSlug ?? previous?.projectSlug,
            targetFolder: item.targetFolder ?? previous?.targetFolder,
            hashes: item.hashes?.length ? item.hashes : previous?.hashes,
            fileLength: item.fileLength ?? previous?.fileLength,
            fileFingerprint: item.fileFingerprint ?? previous?.fileFingerprint,
            ownershipKind: item.ownershipKind ?? previous?.ownershipKind,
            operationKind: item.operationKind ?? previous?.operationKind,
        })
    }
    manualDownloadsByInstance.set(instanceId, [...deduped.values()])
}

export function getCurseForgeManualDownloadUrl(item: CurseForgeManualDownloadItem) {
    const projectTypePath = {
        mod: 'mc-mods',
        modpack: 'modpacks',
        datapack: 'data-packs',
        resourcepack: 'texture-packs',
        shader: 'shaders',
        shaderpack: 'shaders',
        world: 'worlds',
    }[item.projectType ?? '']
    const fallback =
        item.projectSlug && projectTypePath
            ? `https://www.curseforge.com/minecraft/${projectTypePath}/${item.projectSlug}/download/${item.fileId}`
            : `https://www.curseforge.com/minecraft/search?search=${encodeURIComponent(item.fileName)}`
    if (!item.websiteUrl) return fallback

    try {
        const url = new URL(item.websiteUrl)
        if (
            !['curseforge.com', 'www.curseforge.com', 'legacy.curseforge.com'].includes(
                url.hostname,
            )
        ) {
            return item.websiteUrl
        }
        const projectPath = url.pathname.match(
            /^\/minecraft\/(?:mc-mods|modpacks|data-packs|texture-packs|shaders|worlds)\/([^/]+)/i,
        )
        if (projectPath?.[1] && /^\d+$/.test(projectPath[1])) return fallback
        url.pathname = url.pathname.replace(
            /\/(?:files|download)\/\d+\/?$/i,
            `/download/${item.fileId}`,
        )
        if (!url.pathname.endsWith(`/download/${item.fileId}`)) {
            url.pathname = `${url.pathname.replace(/\/$/, '')}/download/${item.fileId}`
        }
        url.search = ''
        url.hash = ''
        return url.toString()
    } catch {
        return fallback
    }
}

export function clearCurseForgeManualDownloads(instanceId: string) {
    setCurseForgeManualDownloads(instanceId, [])
}

export function filterInstalledCurseForgeManualDownloads(
    manualDownloads: CurseForgeManualDownloadItem[],
    installedItems: InstalledCurseForgeContentItem[],
) {
    const installedFileNames = new Set(
        installedItems.flatMap((item) => {
            const lower = item.file_name.toLowerCase()
            const base = lower.replace(/\.disabled$/i, '')
            return [lower, base].filter(Boolean)
        }),
    )
    const installedFileFamilies = new Set(
        installedItems
            .map((item) => modFileFamily(item.file_name))
            .filter((family): family is string => !!family),
    )
    const installedCurseForgeProjects = new Set(
        installedItems.flatMap((item) =>
            (item.provider_refs ?? [])
                .filter((reference) => reference.provider === 'curseforge')
                .map((reference) => reference.project_id),
        ),
    )
    const installedCurseForgeFiles = new Set(
        installedItems.flatMap((item) =>
            (item.provider_refs ?? [])
                .filter(
                    (
                        reference,
                    ): reference is Extract<
                        NonNullable<InstalledCurseForgeContentItem['provider_refs']>[number],
                        { provider: 'curseforge' }
                    > => reference.provider === 'curseforge' && reference.file_id != null,
                )
                .map((reference) => `${reference.project_id}:${reference.file_id}`),
        ),
    )
    return manualDownloads.filter((item) => {
        const fileFamily = modFileFamily(item.fileName)
        return (
            !installedCurseForgeProjects.has(item.projectId) &&
            !installedCurseForgeFiles.has(`${item.projectId}:${item.fileId}`) &&
            !installedFileNames.has(item.fileName.toLowerCase()) &&
            (!fileFamily || !installedFileFamilies.has(fileFamily))
        )
    })
}

export function removeInstalledCurseForgeManualDownloads(
    instanceId: string,
    manualDownloads: CurseForgeManualDownloadItem[],
    installedItems: InstalledCurseForgeContentItem[],
) {
    const remaining = filterInstalledCurseForgeManualDownloads(manualDownloads, installedItems)
    setCurseForgeManualDownloads(instanceId, remaining)
    return remaining
}
