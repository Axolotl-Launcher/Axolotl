import type { ItemRef } from './identifier.ts'
import { rawId } from './identifier.ts'
import type { JavaVersionId } from './types.ts'
import { isVersionAtLeast } from './versions.ts'

export interface JavaRecipeFormatter {
    typeName(base: string): string
    ingredient(ref: ItemRef, isTag: boolean): unknown
    result(ref: ItemRef, count?: number): Record<string, unknown>
    cookingResult(ref: ItemRef, count?: number): unknown
    stonecutterResult(ref: ItemRef, count?: number): Record<string, unknown>
}

function withCount<T extends Record<string, unknown>>(
    value: T,
    count: number | undefined,
): T | (T & { count: number }) {
    return typeof count === 'number' && count > 0 ? { ...value, count } : value
}

const bareType = (base: string) => base
const namespacedType = (base: string) => `minecraft:${base}`

const v112: JavaRecipeFormatter = {
    typeName: bareType,
    ingredient: (ref) => ({
        item: rawId(ref),
        ...(ref.data !== undefined ? { data: ref.data } : {}),
    }),
    result: (ref, count) =>
        withCount(
            { item: rawId(ref), ...(ref.data !== undefined ? { data: ref.data } : {}) },
            count,
        ),
    cookingResult: (ref) => rawId(ref),
    stonecutterResult: (ref, count) => ({ result: rawId(ref), count: count ?? 1 }),
}

const v113: JavaRecipeFormatter = {
    typeName: bareType,
    ingredient: (ref, isTag) => (isTag ? { tag: rawId(ref) } : { item: rawId(ref) }),
    result: (ref, count) => withCount({ item: rawId(ref) }, count),
    cookingResult: (ref) => rawId(ref),
    stonecutterResult: (ref, count) => ({ result: rawId(ref), count: count ?? 1 }),
}

const v114: JavaRecipeFormatter = {
    ...v113,
    typeName: namespacedType,
}

const v120: JavaRecipeFormatter = {
    ...v114,
    result: (ref, count) => withCount({ id: rawId(ref) }, count),
    cookingResult: (ref, count) => withCount({ id: rawId(ref) }, count),
    stonecutterResult: (ref, count) => ({ result: withCount({ id: rawId(ref) }, count) }),
}

const v1212: JavaRecipeFormatter = {
    ...v120,
    ingredient: (ref, isTag) => (isTag ? `#${rawId(ref)}` : rawId(ref)),
}

export function createJavaFormatter(version: JavaVersionId): JavaRecipeFormatter {
    if (version === '1.12') return v112
    if (version === '1.13') return v113
    if (isVersionAtLeast(version, '1.21.2')) return v1212
    if (isVersionAtLeast(version, '1.20')) return v120
    return v114
}
