export const clientInstallableLoaders = [
    'fabric',
    'neoforge',
    'forge',
    'quilt',
    'optifine',
    'cleanroom',
    'lite_loader',
    'legacy_fabric',
    'babric',
] as const

export const instanceInstallablePlatforms = ['vanilla', ...clientInstallableLoaders] as const

export const loaderDisplayNames: Record<string, string> = {
    fabric: 'Fabric',
    neoforge: 'NeoForge',
    neo_forge: 'NeoForge',
    forge: 'Forge',
    quilt: 'Quilt',
    paper: 'Paper',
    spigot: 'Spigot',
    purpur: 'Purpur',
    bukkit: 'Bukkit',
    vanilla: 'Vanilla',
    lite_loader: 'LiteLoader',
    cleanroom: 'Cleanroom',
    legacy_fabric: 'Legacy Fabric',
    babric: 'Babric',
    optifine: 'OptiFine',
}

export const loaderMessages: Record<string, { id: string; defaultMessage: string }> = {
    vanilla: {
        id: 'loader.vanilla',
        defaultMessage: 'None',
    },
}

export const formatLoaderLabel = (
    item: string,
    formatMessage?: (msg: { id: string; defaultMessage: string }) => string,
) => {
    if (formatMessage && loaderMessages[item]) {
        return formatMessage(loaderMessages[item])
    }
    return loaderDisplayNames[item] ?? item.charAt(0).toUpperCase() + item.slice(1)
}

function concreteLoaderVersion(loaderVersion: string | null): string | null {
    return loaderVersion && loaderVersion !== 'latest' && loaderVersion !== 'stable'
        ? loaderVersion
        : null
}

export function defaultInstanceName(
    loader: string | null,
    gameVersion: string,
    loaderVersion: string | null = null,
): string {
    const loaderLabel = loader ? formatLoaderLabel(loader) : 'Vanilla'
    const exactLoaderVersion = concreteLoaderVersion(loaderVersion)
    return `${gameVersion}-${loaderLabel}${exactLoaderVersion ? ` ${exactLoaderVersion}` : ''}`
}

export function buildUpgradeDisplayNames(input: {
    sourceName: string
    sourceLoader: string
    sourceGameVersion: string
    sourceLoaderVersion: string | null
    targetLoader: string
    targetGameVersion: string
    targetLoaderVersion: string | null
    backupName: string
    customCopyName: string
}) {
    const sourceUsesDefaultName =
        input.sourceName ===
        defaultInstanceName(input.sourceLoader, input.sourceGameVersion, input.sourceLoaderVersion)
    const targetDefault = defaultInstanceName(
        input.targetLoader,
        input.targetGameVersion,
        input.targetLoaderVersion,
    )
    return {
        backup: input.backupName,
        copy: sourceUsesDefaultName ? targetDefault : input.customCopyName,
        upgradedTarget: sourceUsesDefaultName ? targetDefault : null,
        shouldAutoRename: sourceUsesDefaultName,
    }
}
