import type { SeedMapLoot } from './advanced.ts'

export function mergeSeedMapLootItems(items: NonNullable<SeedMapLoot['items']>) {
    const merged = new Map<string, (typeof items)[number]>()
    for (const item of items) {
        const key = JSON.stringify([
            item.id,
            [...item.enchantments]
                .map((enchantment) => [enchantment.id, enchantment.level])
                .sort(([leftId, leftLevel], [rightId, rightLevel]) =>
                    `${leftId}:${leftLevel}`.localeCompare(`${rightId}:${rightLevel}`),
                ),
            item.effect ?? null,
            item.duration ?? null,
        ])
        const existing = merged.get(key)
        if (existing) existing.count += item.count
        else
            merged.set(key, {
                ...item,
                enchantments: item.enchantments.map((value) => ({ ...value })),
            })
    }
    return [...merged.values()]
}
