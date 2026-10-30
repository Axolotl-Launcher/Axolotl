import { type NbtCompound, NbtTag } from 'deepslate/nbt'
import { StringReader } from 'deepslate/util'

import {
    type DyeColor,
    dyeColorDefinition,
    dyeColorFromLegacyIndex,
    normalizeDyeColor,
} from './colors.ts'
import {
    bannerPattern,
    type BannerPatternId,
    MAX_BANNER_LAYERS,
    normalizeBannerPatternId,
} from './patterns.ts'
import { type BannerDesign, type BannerLayer, createBannerLayer } from './types.ts'

export const BANNER_DESIGN_FORMAT = 'axolotl-banner'
export const BANNER_DESIGN_VERSION = 1

/**
 * Largest import the parser accepts. A six layer design is a few hundred bytes,
 * so anything past this is either a mistake or an attempt to make the renderer
 * chew through a file it has no use for.
 */
export const MAX_IMPORT_BYTES = 256 * 1024

export type BannerParseErrorCode =
    'empty' | 'unsupported' | 'too-large' | 'invalid-color' | 'invalid-pattern' | 'invalid-data'

export class BannerParseError extends Error {
    readonly code: BannerParseErrorCode
    readonly detail?: string

    constructor(code: BannerParseErrorCode, detail?: string) {
        super(detail ? `${code}: ${detail}` : code)
        this.name = 'BannerParseError'
        this.code = code
        this.detail = detail
    }
}

export function toDesignJson(design: BannerDesign): string {
    return `${JSON.stringify(
        {
            format: BANNER_DESIGN_FORMAT,
            version: BANNER_DESIGN_VERSION,
            baseColor: design.baseColor,
            layers: design.layers.map((layer) => ({
                pattern: layer.pattern,
                color: layer.color,
            })),
        },
        null,
        2,
    )}\n`
}

/** `[{pattern:"minecraft:stripe_bottom",color:"red"}]`, empty when the banner is plain. */
export function toPatternsComponent(design: BannerDesign): string {
    if (design.layers.length === 0) return '[]'
    const entries = design.layers.map(
        (layer) => `{pattern:"minecraft:${layer.pattern}",color:"${layer.color}"}`,
    )
    return `[${entries.join(',')}]`
}

export interface BannerCommandOptions {
    target?: string
    count?: number
    /** Item data dialect to emit; defaults to the 1.20.5+ components. */
    version?: BannerGameVersion
}

/**
 * Item data dialects: the 1.20.5+ components, the item NBT that 1.13 through
 * 1.20.4 reads, or the single banner item with a damage value that 1.12.2 and
 * older take.
 */
export type BannerGameVersion = 'modern' | 'legacy' | 'classic'

/** Newest game version that still reads the legacy item NBT. */
export const BANNER_LEGACY_MAX_VERSION = '1.20.4'

/** Newest game version that still has one banner item for every dye. */
export const BANNER_CLASSIC_MAX_VERSION = '1.12.2'

/** The item 1.12.2 and older use for every dye; the color lives in the damage value. */
const CLASSIC_BANNER_ITEM = 'minecraft:banner'

/** Compares dotted version strings, treating missing segments as zero. */
function compareGameVersions(left: string, right: string): number {
    const leftParts = left.split('.').map((part) => Number.parseInt(part, 10) || 0)
    const rightParts = right.split('.').map((part) => Number.parseInt(part, 10) || 0)
    for (let index = 0; index < Math.max(leftParts.length, rightParts.length); index++) {
        const difference = (leftParts[index] ?? 0) - (rightParts[index] ?? 0)
        if (difference !== 0) return difference
    }
    return 0
}

/**
 * Patterns the chosen export target cannot carry. The game resolves pattern ids
 * against its own registry and skips unknown ones without an error, so a banner
 * built from newer patterns arrives plain unless the caller warns about them.
 */
export function unsupportedPatterns(
    design: BannerDesign,
    version: BannerGameVersion,
): BannerPatternId[] {
    const newest =
        version === 'modern'
            ? null
            : version === 'classic'
              ? BANNER_CLASSIC_MAX_VERSION
              : BANNER_LEGACY_MAX_VERSION
    if (!newest) return []
    const missing: BannerPatternId[] = []
    for (const layer of design.layers) {
        const since = bannerPattern(layer.pattern).since
        if (!since || compareGameVersions(since, newest) <= 0) continue
        if (!missing.includes(layer.pattern)) missing.push(layer.pattern)
    }
    return missing
}

export function toGiveCommand(design: BannerDesign, options: BannerCommandOptions = {}): string {
    const target = options.target?.trim() || '@p'
    const count = Math.max(1, Math.trunc(options.count ?? 1))
    const version = options.version ?? 'modern'
    const data = toItemData(design, { version })
    if (version === 'classic') {
        // 1.12 and older take the count, a damage value and the tag after the item.
        return `/give ${target} ${CLASSIC_BANNER_ITEM} ${count} 0${data ? ` ${data}` : ''}`
    }
    const item = dyeColorDefinition(design.baseColor).bannerItem
    // The component or NBT hangs off the item id with no space in between, so a
    // plain banner still renders as `minecraft:blue_banner 1`, never with two.
    return `/give ${target} ${item}${data} ${count}`
}

export function toItemData(
    design: BannerDesign,
    options: { version?: BannerGameVersion } = {},
): string {
    const version = options.version ?? 'modern'
    if (version === 'modern') {
        return design.layers.length === 0
            ? ''
            : `[minecraft:banner_patterns=${toPatternsComponent(design)}]`
    }
    // Every pre-1.20.5 version stores the same block entity tag.
    const base = dyeColorDefinition(design.baseColor).legacyIndex
    // Those versions also store a short code here. Patterns added after the 1.13
    // registry rename have no code of their own, so they fall back to the id.
    const patterns = design.layers
        .map((layer) => {
            const color = `${dyeColorDefinition(layer.color).legacyIndex}b`
            const code = bannerPattern(layer.pattern).legacyCode ?? layer.pattern
            return `{Pattern:"${code}",Color:${color}}`
        })
        .join(',')
    return `{BlockEntityTag:{Base:${base}b,Patterns:[${patterns}]}}`
}

function requirePattern(value: string | undefined): BannerPatternId {
    const pattern = value ? normalizeBannerPatternId(value) : null
    if (!pattern) throw new BannerParseError('invalid-pattern', value)
    return pattern
}

function clampLayers(layers: BannerLayer[]): BannerLayer[] {
    return layers.length > MAX_BANNER_LAYERS ? layers.slice(0, MAX_BANNER_LAYERS) : layers
}

export function parseBannerText(text: string): BannerDesign {
    const trimmed = text.trim()
    if (!trimmed) throw new BannerParseError('empty')
    // `length` counts UTF-16 code units, which is never more than the encoded
    // size, so this rejects the worst offenders before anything is encoded.
    if (trimmed.length > MAX_IMPORT_BYTES) {
        throw new BannerParseError('too-large', String(trimmed.length))
    }
    const byteLength = new TextEncoder().encode(trimmed).length
    if (byteLength > MAX_IMPORT_BYTES) {
        throw new BannerParseError('too-large', String(byteLength))
    }

    if (/^\/?\s*give\b/i.test(trimmed)) return parseGiveCommand(trimmed)
    if (trimmed.startsWith('[')) return fromComponentData(trimmed, 'white')
    if (trimmed.startsWith('{')) {
        const json = tryParseJson(trimmed)
        if (json && isDesignFile(json)) return fromDesignFile(json)
        return fromSnbt(trimmed, 'white')
    }
    throw new BannerParseError('unsupported')
}

function tryParseJson(text: string): Record<string, unknown> | null {
    try {
        const value: unknown = JSON.parse(text)
        return value && typeof value === 'object' && !Array.isArray(value)
            ? (value as Record<string, unknown>)
            : null
    } catch {
        return null
    }
}

function isDesignFile(value: Record<string, unknown>): boolean {
    return (
        value.format === BANNER_DESIGN_FORMAT ||
        'baseColor' in value ||
        'layers' in value ||
        ('base' in value && !('BlockEntityTag' in value))
    )
}

function fromDesignFile(value: Record<string, unknown>): BannerDesign {
    const rawLayers = Array.isArray(value.layers)
        ? value.layers
        : Array.isArray(value.patterns)
          ? value.patterns
          : []
    const layers: BannerLayer[] = []
    for (const entry of rawLayers) {
        // A banner cannot hold more layers than the survival limit, so reading
        // further entries would only allocate what the game would drop.
        if (layers.length >= MAX_BANNER_LAYERS) break
        if (!entry || typeof entry !== 'object') continue
        const record = entry as Record<string, unknown>
        const patternValue = record.pattern ?? record.Pattern
        const colorValue = record.color ?? record.Color
        const color =
            typeof colorValue === 'number'
                ? dyeColorFromLegacyIndex(colorValue)
                : typeof colorValue === 'string'
                  ? normalizeDyeColor(colorValue)
                  : null
        if (!color) throw new BannerParseError('invalid-color', String(colorValue))
        layers.push(
            createBannerLayer(
                requirePattern(typeof patternValue === 'string' ? patternValue : undefined),
                color,
            ),
        )
    }
    const baseValue = value.baseColor ?? value.base
    const baseColor =
        typeof baseValue === 'number'
            ? dyeColorFromLegacyIndex(baseValue)
            : typeof baseValue === 'string'
              ? normalizeDyeColor(baseValue)
              : null
    if (baseValue !== undefined && !baseColor) {
        throw new BannerParseError('invalid-color', String(baseValue))
    }
    return { baseColor: baseColor ?? 'white', layers: clampLayers(layers) }
}

/**
 * Parses a bare compound by wrapping it in one, since SNBT documents need a
 * compound root.
 */
function parseSnbtRoot(text: string): NbtTag {
    try {
        const root = NbtTag.fromString(`{value:${text}}`)
        if (!root.isCompound()) throw new BannerParseError('invalid-data')
        const value = root.get('value')
        if (!value) throw new BannerParseError('invalid-data')
        return value
    } catch (error) {
        if (error instanceof BannerParseError) throw error
        throw new BannerParseError('invalid-data', (error as Error).message)
    }
}

/**
 * Item data components use the `[key=value,…]` command syntax, which is not
 * SNBT; each value is parsed as an SNBT tag.
 */
function parseComponents(text: string): Map<string, NbtTag> {
    const components = new Map<string, NbtTag>()
    try {
        const reader = new StringReader(text)
        reader.skipWhitespace()
        reader.expect('[')
        reader.skipWhitespace()
        while (reader.canRead() && reader.peek() !== ']') {
            const start = reader.cursor
            while (reader.canRead() && /[a-z0-9_:./-]/i.test(reader.peek())) reader.skip()
            const name = reader.getRead(start)
            if (!name) throw new BannerParseError('invalid-data', 'missing component name')
            reader.skipWhitespace()
            reader.expect('=')
            components.set(name, NbtTag.fromString(reader))
            reader.skipWhitespace()
            if (!reader.canRead() || reader.peek() === ']') break
            reader.expect(',')
            reader.skipWhitespace()
        }
        reader.expect(']')
    } catch (error) {
        if (error instanceof BannerParseError) throw error
        throw new BannerParseError('invalid-data', (error as Error).message)
    }
    return components
}

function fromComponentData(text: string, fallbackColor: DyeColor): BannerDesign {
    const components = parseComponents(text)
    for (const [name, tag] of components) {
        if (name !== 'banner_patterns' && name !== 'minecraft:banner_patterns') continue
        return { baseColor: fallbackColor, layers: clampLayers(readPatternList(tag)) }
    }
    throw new BannerParseError('unsupported')
}

function fromSnbt(text: string, fallbackColor: DyeColor): BannerDesign {
    const tag = parseSnbtRoot(text)
    if (tag.isCompound()) {
        const compound = tag.hasCompound('BlockEntityTag') ? tag.getCompound('BlockEntityTag') : tag
        if (compound.has('Base') || compound.has('Patterns')) {
            return fromLegacyCompound(compound, fallbackColor)
        }
        const layers = readComponentPatterns(compound)
        if (layers) return { baseColor: fallbackColor, layers: clampLayers(layers) }
        throw new BannerParseError('unsupported')
    }
    throw new BannerParseError('unsupported')
}

function readComponentPatterns(compound: NbtCompound): BannerLayer[] | null {
    const key = compound.has('banner_patterns')
        ? 'banner_patterns'
        : compound.has('minecraft:banner_patterns')
          ? 'minecraft:banner_patterns'
          : null
    if (!key) return null
    return readPatternList(compound.get(key))
}

function readPatternList(tag: NbtTag | undefined): BannerLayer[] {
    if (!tag || !tag.isList()) return []
    const layers: BannerLayer[] = []
    for (const entry of tag.getItems()) {
        // Stop at the layer limit instead of building a list the game would cut.
        if (layers.length >= MAX_BANNER_LAYERS) break
        if (!entry.isCompound()) continue
        const patternTag = entry.get('pattern') ?? entry.get('Pattern')
        if (!patternTag) continue
        const colorTag = entry.get('color') ?? entry.get('Color')
        const color = colorTag
            ? colorTag.isString()
                ? normalizeDyeColor(colorTag.getAsString())
                : colorTag.isNumber()
                  ? dyeColorFromLegacyIndex(colorTag.getAsNumber())
                  : null
            : null
        if (!color) throw new BannerParseError('invalid-color', colorTag?.toString())
        layers.push(createBannerLayer(requirePattern(patternTag.getAsString()), color))
    }
    return layers
}

function fromLegacyCompound(compound: NbtCompound, fallbackColor: DyeColor): BannerDesign {
    let baseColor = fallbackColor
    if (compound.has('Base')) {
        const legacyBase = dyeColorFromLegacyIndex(compound.getNumber('Base'))
        if (!legacyBase) {
            throw new BannerParseError('invalid-color', String(compound.getNumber('Base')))
        }
        baseColor = legacyBase
    }
    const patterns = compound.has('Patterns') ? readPatternList(compound.get('Patterns')) : []
    return { baseColor, layers: clampLayers(patterns) }
}

function readBalanced(text: string, open: string, close: string): string {
    let depth = 0
    let quote: string | null = null
    for (let index = 0; index < text.length; index++) {
        const character = text[index]
        if (quote) {
            if (character === '\\') index++
            else if (character === quote) quote = null
            continue
        }
        if (character === '"' || character === "'") {
            quote = character
            continue
        }
        if (character === open) depth++
        else if (character === close) {
            depth--
            if (depth === 0) return text.slice(0, index + 1)
        }
    }
    throw new BannerParseError('invalid-data', 'unbalanced brackets')
}

/**
 * Reads the dye a give command's item stands for. 1.13 split the banner into one
 * item per dye; before that the single `minecraft:banner` carried its color in
 * the damage value instead. Anything that is not a banner is rejected here, so a
 * command for a diamond is not silently imported as a plain white banner.
 */
function itemColorFromId(itemId: string, damage: number): DyeColor {
    const normalized = itemId.replace(/^minecraft:/i, '').toLowerCase()
    if (normalized === 'banner') {
        const color = dyeColorFromLegacyIndex(damage)
        if (!color) throw new BannerParseError('invalid-color', String(damage))
        return color
    }
    const color = normalized.endsWith('_banner')
        ? normalizeDyeColor(normalized.slice(0, -'_banner'.length))
        : null
    if (!color) throw new BannerParseError('unsupported', itemId)
    return color
}

/**
 * Reads the selector a give command targets. Selectors can carry spaces inside
 * quotes or brackets, as in `@a[name="a b"]`, so the token ends at the first
 * whitespace that is neither quoted nor nested.
 */
function readTarget(text: string): string | null {
    let depth = 0
    let quote: string | null = null
    for (let index = 0; index < text.length; index++) {
        const character = text[index]
        if (quote) {
            if (character === '\\') index++
            else if (character === quote) quote = null
            continue
        }
        if (character === '"' || character === "'") {
            quote = character
        } else if (character === '[' || character === '{') {
            depth++
        } else if (character === ']' || character === '}') {
            depth--
        } else if (depth <= 0 && /\s/.test(character)) {
            return text.slice(0, index)
        }
    }
    return null
}

function parseGiveCommand(command: string): BannerDesign {
    let rest = command.replace(/^\/\s*/, '').replace(/^give\s+/i, '')
    const target = readTarget(rest)
    if (!target) throw new BannerParseError('invalid-data', 'missing item')
    rest = rest.slice(target.length).trimStart()

    const item = /^[a-z0-9_.:-]+/i.exec(rest)
    if (!item) throw new BannerParseError('invalid-data', 'missing item')
    const itemId = item[0]
    rest = rest.slice(itemId.length)

    let data: string | null = null
    let damage = 0
    if (rest.startsWith('[') || rest.startsWith('{')) {
        data = rest.startsWith('[') ? readBalanced(rest, '[', ']') : readBalanced(rest, '{', '}')
    } else {
        // 1.12 and older order the arguments as count, damage value, then data.
        const start = rest.search(/[[{]/)
        const numbers = (start >= 0 ? rest.slice(0, start) : rest).match(/\d+/g) ?? []
        if (numbers.length > 1) damage = Number.parseInt(numbers[1], 10)
        if (start >= 0) {
            const tail = rest.slice(start)
            data = tail.startsWith('[')
                ? readBalanced(tail, '[', ']')
                : readBalanced(tail, '{', '}')
        }
    }

    const itemColor = itemColorFromId(itemId, damage)

    if (!data) return { baseColor: itemColor, layers: [] }
    if (data.startsWith('[')) return fromComponentData(data, itemColor)
    return fromSnbt(data, itemColor)
}
