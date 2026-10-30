import type { JavaVersionId, PackFormatVersion } from './types.ts'

export type JavaVersionMeta = {
    id: JavaVersionId
    packFormat: PackFormatVersion | null
    recipeDir: 'recipe' | 'recipes' | null
    tagDir: 'tags/item' | 'tags/items' | null
    hasVanillaTags: boolean
}

const metadata: Record<JavaVersionId, Omit<JavaVersionMeta, 'id' | 'hasVanillaTags'>> = {
    '1.12': { packFormat: null, recipeDir: null, tagDir: null },
    '1.13': { packFormat: 4, recipeDir: 'recipes', tagDir: 'tags/items' },
    '1.14': { packFormat: 4, recipeDir: 'recipes', tagDir: 'tags/items' },
    '1.15': { packFormat: 5, recipeDir: 'recipes', tagDir: 'tags/items' },
    '1.16': { packFormat: 6, recipeDir: 'recipes', tagDir: 'tags/items' },
    '1.17': { packFormat: 7, recipeDir: 'recipes', tagDir: 'tags/items' },
    '1.18': { packFormat: 9, recipeDir: 'recipes', tagDir: 'tags/items' },
    '1.19': { packFormat: 12, recipeDir: 'recipes', tagDir: 'tags/items' },
    '1.20': { packFormat: 41, recipeDir: 'recipes', tagDir: 'tags/items' },
    '1.21': { packFormat: 48, recipeDir: 'recipe', tagDir: 'tags/item' },
    '1.21.2': { packFormat: 57, recipeDir: 'recipe', tagDir: 'tags/item' },
    '1.21.4': { packFormat: 61, recipeDir: 'recipe', tagDir: 'tags/item' },
    '1.21.5': { packFormat: 71, recipeDir: 'recipe', tagDir: 'tags/item' },
    '1.21.6': { packFormat: 80, recipeDir: 'recipe', tagDir: 'tags/item' },
    '1.21.7': { packFormat: 81, recipeDir: 'recipe', tagDir: 'tags/item' },
    '1.21.9': { packFormat: [88, 0], recipeDir: 'recipe', tagDir: 'tags/item' },
    '1.21.11': { packFormat: [94, 1], recipeDir: 'recipe', tagDir: 'tags/item' },
    '26.1': { packFormat: [101, 1], recipeDir: 'recipe', tagDir: 'tags/item' },
    '26.2': { packFormat: [107, 1], recipeDir: 'recipe', tagDir: 'tags/item' },
    '26.3': { packFormat: [121, 0], recipeDir: 'recipe', tagDir: 'tags/item' },
}

const tagVersions = new Set([
    '1.14',
    '1.15',
    '1.16',
    '1.17',
    '1.18',
    '1.19',
    '1.20',
    '1.21',
    '1.21.2',
    '1.21.4',
    '1.21.5',
    '1.21.6',
    '1.21.7',
    '1.21.9',
    '1.21.11',
    '26.1',
    '26.2',
    '26.3',
])

export const JAVA_VERSIONS: readonly JavaVersionMeta[] = (
    [
        '26.3',
        '26.2',
        '26.1',
        '1.21.11',
        '1.21.9',
        '1.21.7',
        '1.21.6',
        '1.21.5',
        '1.21.4',
        '1.21.2',
        '1.21',
        '1.20',
        '1.19',
        '1.18',
        '1.17',
        '1.16',
        '1.15',
        '1.14',
        '1.13',
        '1.12',
    ] as const
).map((id) => ({
    id,
    ...metadata[id],
    hasVanillaTags: tagVersions.has(id),
}))

export const LATEST_JAVA_VERSION: JavaVersionId = '26.3'

const byId = new Map<JavaVersionId, JavaVersionMeta>(
    JAVA_VERSIONS.map((version) => [version.id, version]),
)

export function getJavaVersionMeta(version: JavaVersionId): JavaVersionMeta {
    const meta = byId.get(version)
    if (!meta) throw new Error(`Unknown Java version: ${version}`)
    return meta
}

export function isJavaVersionId(value: unknown): value is JavaVersionId {
    return typeof value === 'string' && byId.has(value as JavaVersionId)
}

export function compareMinecraftVersions(a: string, b: string): number {
    const aParts = a.split('.').map(Number)
    const bParts = b.split('.').map(Number)
    for (let index = 0; index < Math.max(aParts.length, bParts.length); index += 1) {
        const diff = (aParts[index] ?? 0) - (bParts[index] ?? 0)
        if (diff !== 0) return diff > 0 ? 1 : -1
    }
    return 0
}

export function isVersionAtLeast(version: JavaVersionId, minimum: string): boolean {
    return compareMinecraftVersions(version, minimum) >= 0
}
