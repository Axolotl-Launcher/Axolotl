export interface BrowseProjectTabLabels {
    modpacks: string
    mods: string
    resourcepacks: string
    datapacks: string
    maps: string
    shaders: string
    servers: string
    schematics: string
    favorites: string
}

export interface BrowseProjectTab {
    label: string
    href: string
    shown?: boolean
    onboardingId?: string
}

export interface BrowseProjectTabOptions {
    modpacks?: boolean
    mods?: boolean
    datapacks?: boolean
    servers?: boolean
    schematic?: boolean
    favorites?: boolean
}

export interface BrowseProjectTabVisibilityInput {
    instance?: {
        game_version?: string
        loader?: string
    } | null
    hasInstanceContext?: boolean
    isServerInstance?: boolean
}

export function supportsDataPacks(gameVersion: string | undefined): boolean {
    const match = gameVersion?.match(/^1\.(\d+)/)
    return match ? Number(match[1]) >= 13 : false
}

export function getBrowseProjectTabOptions({
    instance,
    hasInstanceContext = false,
    isServerInstance = false,
}: BrowseProjectTabVisibilityInput): BrowseProjectTabOptions {
    const hasInstance = !!instance
    return {
        modpacks: !hasInstanceContext,
        mods: !hasInstance || instance?.loader !== 'vanilla',
        datapacks: !hasInstance || (!isServerInstance && supportsDataPacks(instance?.game_version)),
        servers: !hasInstanceContext,
    }
}

export function createBrowseProjectTabs(
    labels: BrowseProjectTabLabels,
    suffix = '',
    options: BrowseProjectTabOptions = {},
): BrowseProjectTab[] {
    return [
        {
            label: labels.modpacks,
            href: `/browse/modpack${suffix}`,
            shown: options.modpacks ?? true,
        },
        {
            label: labels.mods,
            href: `/browse/mod${suffix}`,
            shown: options.mods ?? true,
        },
        { label: labels.resourcepacks, href: `/browse/resourcepack${suffix}` },
        {
            label: labels.datapacks,
            href: `/browse/datapack${suffix}`,
            shown: options.datapacks ?? true,
        },
        { label: labels.maps, href: `/browse/world${suffix}` },
        { label: labels.shaders, href: `/browse/shader${suffix}` },
        {
            label: labels.servers,
            href: `/browse/server${suffix}`,
            shown: options.servers ?? true,
        },
        {
            label: labels.schematics,
            href: `/browse/schematic${suffix}`,
            shown: options.schematic ?? true,
        },
        {
            label: labels.favorites,
            href: `/browse/favorites${suffix}`,
            shown: options.favorites ?? true,
            onboardingId: 'browse-favorites-tab',
        },
    ]
}
