import type { SchematicBlockState } from './backend.ts'

export type SchematicStateProperty = {
    key: string
    value: string
    values: string[]
    boolean: boolean
}

const PROPERTY_ORDER = [
    'facing',
    'axis',
    'half',
    'type',
    'shape',
    'face',
    'attachment',
    'hinge',
    'part',
    'orientation',
    'rotation',
    'vertical_direction',
    'tilt',
    'waterlogged',
]

const VALUE_ORDER = [
    'false',
    'true',
    'none',
    'bottom',
    'lower',
    'top',
    'upper',
    'double',
    'floor',
    'wall',
    'ceiling',
    'straight',
    'inner_left',
    'inner_right',
    'outer_left',
    'outer_right',
    'left',
    'right',
    'side',
    'north',
    'east',
    'south',
    'west',
    'up',
    'down',
    'single',
    'single_wall',
    'middle_wall',
    'double_wall',
]

function asObject(value: unknown): Record<string, unknown> | undefined {
    return value !== null && typeof value === 'object' && !Array.isArray(value)
        ? (value as Record<string, unknown>)
        : undefined
}

function addValue(values: Map<string, Set<string>>, key: string, value: string) {
    const known = values.get(key)
    if (known) known.add(value)
    else values.set(key, new Set([value]))
}

function collectCondition(condition: unknown, values: Map<string, Set<string>>) {
    if (typeof condition === 'string') {
        for (const pair of condition.split(',')) {
            const [key, value] = pair.split('=')
            if (key && value) addValue(values, key.trim(), value.trim())
        }
        return
    }
    const object = asObject(condition)
    if (!object) return
    for (const [key, value] of Object.entries(object)) {
        if (key === 'OR' || key === 'AND') {
            if (Array.isArray(value)) {
                for (const entry of value) collectCondition(entry, values)
            }
            continue
        }
        if (typeof value !== 'string') continue
        for (const candidate of value.split('|')) {
            const trimmed = candidate.trim()
            if (trimmed) addValue(values, key, trimmed)
        }
    }
}

/**
 * Collects the property values a block definition can be rendered with. Vanilla
 * block states only list the properties that pick a model, so a property missing
 * from both `variants` and `multipart` has no authoritative value set here.
 */
export function collectSchematicStateValues(definition: unknown) {
    const values = new Map<string, Set<string>>()
    const object = asObject(definition)
    const variants = asObject(object?.variants)
    if (variants) {
        for (const variant of Object.keys(variants)) {
            for (const pair of variant.split(',')) {
                const [key, value] = pair.split('=')
                if (key && value) addValue(values, key.trim(), value.trim())
            }
        }
    }
    if (Array.isArray(object?.multipart)) {
        for (const part of object.multipart) collectCondition(asObject(part)?.when, values)
    }
    return values
}

function valueRank(value: string) {
    const rank = VALUE_ORDER.indexOf(value)
    return rank === -1 ? VALUE_ORDER.length : rank
}

function sortStateValues(values: Iterable<string>) {
    return [...values].sort((left, right) => {
        const leftNumber = Number(left)
        const rightNumber = Number(right)
        if (Number.isFinite(leftNumber) && Number.isFinite(rightNumber)) {
            return leftNumber - rightNumber
        }
        const rank = valueRank(left) - valueRank(right)
        return rank || left.localeCompare(right)
    })
}

function propertyRank(key: string) {
    const rank = PROPERTY_ORDER.indexOf(key)
    return rank === -1 ? PROPERTY_ORDER.length : rank
}

/**
 * Describes every property of `state` as an editable row. Values come from the
 * block's own definition, so a state can only be moved to a variant the block
 * actually declares; a property without derivable alternatives is read-only.
 */
export function schematicStateProperties(
    state: SchematicBlockState,
    definition: unknown,
): SchematicStateProperty[] {
    const known = collectSchematicStateValues(definition)
    return Object.entries(state.properties)
        .map(([key, value]) => {
            const boolean = value === 'true' || value === 'false'
            const values = boolean ? new Set(['false', 'true']) : new Set(known.get(key) ?? [])
            values.add(value)
            return { key, value, boolean, values: sortStateValues(values) }
        })
        .sort(
            (left, right) =>
                propertyRank(left.key) - propertyRank(right.key) ||
                left.key.localeCompare(right.key),
        )
}
