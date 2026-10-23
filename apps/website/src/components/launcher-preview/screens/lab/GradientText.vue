<script setup lang="ts">
import ArrowDownIcon from '@modrinth/assets/icons/arrow-down.svg?component'
import ArrowUpIcon from '@modrinth/assets/icons/arrow-up.svg?component'
import AsteriskIcon from '@modrinth/assets/icons/asterisk.svg?component'
import BoldIcon from '@modrinth/assets/icons/bold.svg?component'
import ChevronRightIcon from '@modrinth/assets/icons/chevron-right.svg?component'
import ClipboardCopyIcon from '@modrinth/assets/icons/clipboard-copy.svg?component'
import DownloadIcon from '@modrinth/assets/icons/download.svg?component'
import ItalicIcon from '@modrinth/assets/icons/italic.svg?component'
import PlusIcon from '@modrinth/assets/icons/plus.svg?component'
import RefreshCwIcon from '@modrinth/assets/icons/refresh-cw.svg?component'
import StrikethroughIcon from '@modrinth/assets/icons/strikethrough.svg?component'
import TrashIcon from '@modrinth/assets/icons/trash.svg?component'
import UnderlineIcon from '@modrinth/assets/icons/underline.svg?component'
import UploadIcon from '@modrinth/assets/icons/upload.svg?component'
import Accordion from '@modrinth/ui/src/components/base/Accordion.vue'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import StyledInput from '@modrinth/ui/src/components/base/StyledInput.vue'
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { injectNotificationManager } from '@modrinth/ui/src/providers/web-notifications.ts'
import { type Component, computed, onMounted, onUnmounted, type Ref, ref, shallowRef } from 'vue'

import zhCN from '~/locales/launcher-preview/lab-a.zh-CN'

import { registerPreviewMessages } from '../../lib/i18n'
import { usePreviewDomain } from '../../lib/state'
import PreviewModal from '../../overlays/PreviewModal.vue'

registerPreviewMessages('zh-CN', zhCN)

/* ------------------------------------------------------------------------ *
 * Gradient text model — a verbatim port of
 * apps/app-frontend/src/lab/gradient-text/gradient-text.ts
 * ------------------------------------------------------------------------ */

const TEXT_FORMATS = ['bold', 'italic', 'underlined', 'strikethrough', 'obfuscated'] as const

type TextFormat = (typeof TEXT_FORMATS)[number]

type GradientTextRun = {
    text: string
    formats: TextFormat[]
}

type GradientTextDocument = {
    lines: GradientTextRun[][]
}

type GradientFormatId =
    | 'vanilla'
    | 'vanilla-compatible'
    | 'standard'
    | 'cmi'
    | 'minimessage'
    | 'minimessage-gradient'
    | 'minedown'
    | 'snbt'
    | 'trchat'
    | 'taboolib'
    | 'taboolib-gradient'
    | 'rosegarden-gradient'
    | 'chat-colors'
    | 'motd'
    | 'bbcode'
    | 'json'
    | 'html'
    | 'csv'
    | 'terraria'

type GradientFormatAdapter = {
    id: GradientFormatId
    label: string
    sample: string
    mimeType: string
    extension: string
    supportsVanillaCharacter?: boolean
    supportsSimplify?: boolean
}

type GradientOutputOptions = {
    vanillaCharacter: '&' | '§'
    simplifyGradients: boolean
}

type GradientCharacter = {
    character: string
    color: string | null
    formats: TextFormat[]
    newline?: boolean
}

type Color = {
    red: number
    green: number
    blue: number
}

const DEFAULT_GRADIENT_COLORS = ['#A855F7', '#22C55E']

const DEFAULT_GRADIENT_DOCUMENT: GradientTextDocument = {
    lines: [[{ text: 'Axolotl', formats: [] }]],
}

const gradientFormatAdapters: readonly GradientFormatAdapter[] = [
    {
        id: 'vanilla',
        label: 'Vanilla',
        sample: '&#RRGGBB',
        mimeType: 'text/plain',
        extension: 'txt',
        supportsVanillaCharacter: true,
    },
    {
        id: 'vanilla-compatible',
        label: 'Vanilla compatible',
        sample: '&x&R&R&G&G&B&B',
        mimeType: 'text/plain',
        extension: 'txt',
        supportsVanillaCharacter: true,
    },
    {
        id: 'standard',
        label: 'Standard HEX',
        sample: '#RRGGBB',
        mimeType: 'text/plain',
        extension: 'txt',
    },
    { id: 'cmi', label: 'CMI', sample: '{#RRGGBB}', mimeType: 'text/plain', extension: 'txt' },
    {
        id: 'minimessage',
        label: 'MiniMessage',
        sample: '<#RRGGBB>',
        mimeType: 'text/plain',
        extension: 'txt',
    },
    {
        id: 'minimessage-gradient',
        label: 'MiniMessage gradient',
        sample: '<gradient:#RRGGBB:#RRGGBB>',
        mimeType: 'text/plain',
        extension: 'txt',
        supportsSimplify: true,
    },
    {
        id: 'minedown',
        label: 'MineDown',
        sample: '&#RRGGBB&',
        mimeType: 'text/plain',
        extension: 'txt',
    },
    {
        id: 'snbt',
        label: 'Stringified NBT',
        sample: "{text:'T',color:'#RRGGBB'}",
        mimeType: 'application/json',
        extension: 'snbt',
    },
    {
        id: 'trchat',
        label: 'TrChat',
        sample: '&{#RRGGBB}',
        mimeType: 'text/plain',
        extension: 'txt',
    },
    {
        id: 'taboolib',
        label: 'TabooLib',
        sample: '&{#RRGGBB}',
        mimeType: 'text/plain',
        extension: 'txt',
    },
    {
        id: 'taboolib-gradient',
        label: 'TabooLib gradient',
        sample: '[Text](gradient=#RRGGBB,#RRGGBB)',
        mimeType: 'text/plain',
        extension: 'txt',
        supportsSimplify: true,
    },
    {
        id: 'rosegarden-gradient',
        label: 'RoseGarden gradient',
        sample: '<g:#RRGGBB:#RRGGBB>Text',
        mimeType: 'text/plain',
        extension: 'txt',
        supportsSimplify: true,
    },
    {
        id: 'chat-colors',
        label: 'Chat Colors',
        sample: '[#RRGGBB]',
        mimeType: 'text/plain',
        extension: 'txt',
    },
    { id: 'motd', label: 'MOTD', sample: '\\u00A7x', mimeType: 'text/plain', extension: 'txt' },
    {
        id: 'bbcode',
        label: 'BBCode',
        sample: '[color=#RRGGBB]Text[/color]',
        mimeType: 'text/plain',
        extension: 'txt',
    },
    {
        id: 'json',
        label: 'JSON text component',
        sample: '{"text":"T","color":"#RRGGBB"}',
        mimeType: 'application/json',
        extension: 'json',
    },
    {
        id: 'html',
        label: 'HTML',
        sample: '<span style="color: #RRGGBB">Text</span>',
        mimeType: 'text/html',
        extension: 'html',
    },
    { id: 'csv', label: 'CSV', sample: '#RRGGBB,T', mimeType: 'text/csv', extension: 'csv' },
    {
        id: 'terraria',
        label: 'Terraria',
        sample: '[c/RRGGBB:T]',
        mimeType: 'text/plain',
        extension: 'txt',
    },
]

function cloneGradientDocument(document: GradientTextDocument): GradientTextDocument {
    return {
        lines: document.lines.map((line) =>
            line.map((run) => ({ text: run.text, formats: [...run.formats] })),
        ),
    }
}

function normalizeHexColor(value: string): string | null {
    const match = value.trim().match(/^#?([0-9a-f]{3}|[0-9a-f]{6})$/i)
    if (!match) return null
    const normalized =
        match[1].length === 3
            ? match[1]
                  .split('')
                  .map((part) => `${part}${part}`)
                  .join('')
            : match[1]
    return `#${normalized.toUpperCase()}`
}

function parseGradientColors(value: string): string[] {
    const colors: string[] = []
    const hexMatches = value.match(/#(?:[0-9a-f]{3}|[0-9a-f]{6})\b/gi) ?? []
    for (const match of hexMatches) {
        const color = normalizeHexColor(match)
        if (color) colors.push(color)
    }

    const rgbExpression = /rgba?\(\s*(\d{1,3})\s*,\s*(\d{1,3})\s*,\s*(\d{1,3})/gi
    for (const match of value.matchAll(rgbExpression)) {
        const components = match.slice(1, 4).map(Number)
        if (components.every((component) => component >= 0 && component <= 255)) {
            colors.push(
                colorToHex({ red: components[0], green: components[1], blue: components[2] }),
            )
        }
    }

    return [...new Set(colors)]
}

function randomGradientColor(random = Math.random): string {
    return colorToHex({
        red: Math.floor(random() * 256),
        green: Math.floor(random() * 256),
        blue: Math.floor(random() * 256),
    })
}

function getMinecraftTextShadow(color: string | null): string {
    const normalized = color ? normalizeHexColor(color) : null
    if (!normalized) return '#000000'

    const source = hexToColor(normalized)
    const red = source.red / 255
    const green = source.green / 255
    const blue = source.blue / 255
    const maximum = Math.max(red, green, blue)
    const minimum = Math.min(red, green, blue)
    const delta = maximum - minimum
    const saturation = maximum === 0 ? 0 : delta / maximum
    let hue = 0

    if (delta) {
        if (maximum === red) hue = ((green - blue) / delta) % 6
        else if (maximum === green) hue = (blue - red) / delta + 2
        else hue = (red - green) / delta + 4
        hue = ((hue * 60 + 360) % 360) / 360
    }

    const lightness = maximum / 4
    const chroma = (1 - Math.abs(2 * lightness - 1)) * saturation
    const segment = hue * 6
    const secondary = chroma * (1 - Math.abs((segment % 2) - 1))
    const match = lightness - chroma / 2
    const [shadowRed, shadowGreen, shadowBlue] =
        segment < 1
            ? [chroma, secondary, 0]
            : segment < 2
              ? [secondary, chroma, 0]
              : segment < 3
                ? [0, chroma, secondary]
                : segment < 4
                  ? [0, secondary, chroma]
                  : segment < 5
                    ? [secondary, 0, chroma]
                    : [chroma, 0, secondary]

    return colorToHex({
        red: Math.round((shadowRed + match) * 255),
        green: Math.round((shadowGreen + match) * 255),
        blue: Math.round((shadowBlue + match) * 255),
    })
}

function interpolateGradient(colors: string[], count: number): string[] {
    const normalized = colors
        .map(normalizeHexColor)
        .filter((color): color is string => color !== null)
    if (count <= 0 || normalized.length === 0) return []
    if (normalized.length === 1 || count === 1)
        return Array.from({ length: count }, () => normalized[0])

    const source = normalized.map(hexToColor)
    return Array.from({ length: count }, (_, index) => {
        const position = index / (count - 1)
        const scaled = position * (source.length - 1)
        const startIndex = Math.floor(scaled)
        const endIndex = Math.min(source.length - 1, startIndex + 1)
        const progress = scaled - startIndex
        const start = source[startIndex]
        const end = source[endIndex]
        return colorToHex({
            red: Math.round(start.red + (end.red - start.red) * progress),
            green: Math.round(start.green + (end.green - start.green) * progress),
            blue: Math.round(start.blue + (end.blue - start.blue) * progress),
        })
    })
}

function buildGradientCharacters(
    document: GradientTextDocument,
    colors: string[],
): GradientCharacter[] {
    const safeDocument = normalizeGradientDocument(document)
    const uncoloredCharacterCount = safeDocument.lines.reduce(
        (total, line) =>
            total +
            line.reduce(
                (lineTotal, run) =>
                    lineTotal +
                    Array.from(run.text).filter((character) => !/\s/u.test(character)).length,
                0,
            ),
        0,
    )
    const gradient = interpolateGradient(colors, uncoloredCharacterCount)
    let colorIndex = 0
    const characters: GradientCharacter[] = []

    safeDocument.lines.forEach((line, lineIndex) => {
        line.forEach((run) => {
            for (const character of Array.from(run.text)) {
                const color = /\s/u.test(character) ? null : (gradient[colorIndex++] ?? null)
                characters.push({ character, color, formats: run.formats })
            }
        })
        if (lineIndex < safeDocument.lines.length - 1) {
            characters.push({ character: '\n', color: null, formats: [], newline: true })
        }
    })

    return characters
}

function generateGradientOutput(
    document: GradientTextDocument,
    colors: string[],
    adapterId: GradientFormatId,
    options: GradientOutputOptions,
): string {
    const characters = buildGradientCharacters(document, colors)
    switch (adapterId) {
        case 'vanilla':
            return formatVanillaCharacters(characters, options.vanillaCharacter)
        case 'vanilla-compatible':
            return formatVanillaCompatibleCharacters(characters, options.vanillaCharacter)
        case 'standard':
            return formatEachCharacter(characters, (character) =>
                character.color ? `${character.color}${character.character}` : character.character,
            )
        case 'cmi':
            return formatEachCharacter(
                characters,
                (character) =>
                    character.color
                        ? `{${character.color}}${character.character}`
                        : character.character,
                ampersandFormats,
            )
        case 'minimessage':
            return formatEachCharacter(
                characters,
                (character) =>
                    character.color
                        ? `<${character.color}>${character.character}`
                        : character.character,
                minimessageFormats,
            )
        case 'minimessage-gradient':
            return formatGradientRuns(
                characters,
                colors,
                options.simplifyGradients,
                (text, runColors, formats) =>
                    wrapFormats(
                        `<gradient:${runColors.join(':')}>${text}</gradient>`,
                        formats,
                        minimessageFormats,
                    ),
            )
        case 'minedown':
            return formatEachCharacter(
                characters,
                (character) =>
                    character.color
                        ? `&${character.color}&${character.character}`
                        : character.character,
                minedownFormats,
            )
        case 'snbt':
            return toSnbt(characters)
        case 'trchat':
        case 'taboolib':
            return formatEachCharacter(
                characters,
                (character) =>
                    character.color
                        ? `&{${character.color}}${character.character}`
                        : character.character,
                ampersandFormats,
            )
        case 'taboolib-gradient':
            return formatGradientRuns(
                characters,
                colors,
                options.simplifyGradients,
                (text, runColors, formats) => {
                    const modifiers = formats
                        .map((format) => taboolibFormats[format])
                        .filter(Boolean)
                    return `[${text}](gradient=${runColors.join(',')}${modifiers.length ? `;${modifiers.join(';')}` : ''})`
                },
            )
        case 'rosegarden-gradient':
            return formatGradientRuns(
                characters,
                colors,
                options.simplifyGradients,
                (text, runColors, formats) =>
                    wrapFormats(`<g:${runColors.join(':')}>${text}`, formats, minimessageFormats),
            )
        case 'chat-colors':
            return formatEachCharacter(
                characters,
                (character) =>
                    character.color
                        ? `[${character.color}]${character.character}`
                        : character.character,
                chatColorsFormats,
            )
        case 'motd':
            return formatMotdCharacters(characters)
        case 'bbcode':
            return formatEachCharacter(
                characters,
                (character) =>
                    character.color
                        ? `[color=${character.color}]${character.character}[/color]`
                        : character.character,
                bbcodeFormats,
            )
        case 'json':
            return JSON.stringify(toTextComponents(characters))
        case 'html':
            return toHtml(characters)
        case 'csv':
            return toCsv(characters)
        case 'terraria':
            return formatEachCharacter(characters, (character) =>
                character.color
                    ? `[c/${character.color.slice(1)}:${character.character}]`
                    : character.character,
            )
    }
}

function normalizeGradientDocument(value: GradientTextDocument): GradientTextDocument {
    const lines = Array.isArray(value?.lines) ? value.lines : []
    const normalizedLines = lines.map((line) => {
        if (!Array.isArray(line)) return []
        return line
            .filter((run): run is GradientTextRun => typeof run?.text === 'string')
            .map((run) => ({
                text: run.text,
                formats: TEXT_FORMATS.filter(
                    (format) => Array.isArray(run.formats) && run.formats.includes(format),
                ),
            }))
    })
    return { lines: normalizedLines.length ? normalizedLines : [[{ text: '', formats: [] }]] }
}

const minimessageFormats: Record<TextFormat, readonly [string, string]> = {
    bold: ['<b>', '</b>'],
    italic: ['<i>', '</i>'],
    underlined: ['<u>', '</u>'],
    strikethrough: ['<st>', '</st>'],
    obfuscated: ['<obf>', '</obf>'],
}

const ampersandFormats: Record<TextFormat, readonly [string, string]> = {
    bold: ['&l', ''],
    italic: ['&o', ''],
    underlined: ['&n', ''],
    strikethrough: ['&m', ''],
    obfuscated: ['&k', ''],
}

const minedownFormats: Record<TextFormat, readonly [string, string]> = {
    bold: ['**', '**'],
    italic: ['##', '##'],
    underlined: ['__', '__'],
    strikethrough: ['~~', '~~'],
    obfuscated: ['??', '??'],
}

const chatColorsFormats: Record<TextFormat, readonly [string, string]> = {
    bold: ['**', '**'],
    italic: ['*', '*'],
    underlined: ['__', '__'],
    strikethrough: ['~~', '~~'],
    obfuscated: ['', ''],
}

const bbcodeFormats: Record<TextFormat, readonly [string, string]> = {
    bold: ['[b]', '[/b]'],
    italic: ['[i]', '[/i]'],
    underlined: ['[u]', '[/u]'],
    strikethrough: ['[s]', '[/s]'],
    obfuscated: ['', ''],
}

const taboolibFormats: Record<TextFormat, string> = {
    bold: 'b',
    italic: 'i',
    underlined: 'u',
    strikethrough: 's',
    obfuscated: 'o',
}

function formatEachCharacter(
    characters: GradientCharacter[],
    formatter: (character: GradientCharacter) => string,
    formatMap?: Record<TextFormat, readonly [string, string]>,
): string {
    return characters
        .map((character) => {
            if (character.newline) return '\n'
            const formatted = formatter(character)
            return formatMap ? wrapFormats(formatted, character.formats, formatMap) : formatted
        })
        .join('')
}

function formatLegacyCharacters(
    characters: GradientCharacter[],
    formatter: (character: GradientCharacter) => string,
): string {
    return characters
        .map((character) => {
            if (character.newline) return '\n'
            return formatter(character)
        })
        .join('')
}

function formatVanillaCharacters(
    characters: GradientCharacter[],
    characterCode: '&' | '§',
): string {
    return formatLegacyCharacters(characters, (character) => {
        if (!character.color) return character.character
        return `${characterCode}${character.color}${legacyFormatCodes(character.formats, characterCode)}${character.character}`
    })
}

function formatVanillaCompatibleCharacters(
    characters: GradientCharacter[],
    characterCode: '&' | '§',
): string {
    return formatLegacyCharacters(characters, (character) => {
        if (!character.color) return character.character
        const hexadecimal = character.color
            .slice(1)
            .split('')
            .map((part) => `${characterCode}${part}`)
            .join('')
        return `${characterCode}x${hexadecimal}${legacyFormatCodes(character.formats, characterCode)}${character.character}`
    })
}

function formatMotdCharacters(characters: GradientCharacter[]): string {
    const characterCode = '\\u00A7'
    return formatLegacyCharacters(characters, (character) => {
        if (!character.color) return character.character
        const hexadecimal = character.color
            .slice(1)
            .split('')
            .map((part) => `${characterCode}${part}`)
            .join('')
        return `${characterCode}x${hexadecimal}${legacyFormatCodes(character.formats, characterCode)}${character.character}`
    })
}

function legacyFormatCodes(formats: TextFormat[], characterCode: string): string {
    return formats
        .map((format) => {
            switch (format) {
                case 'bold':
                    return `${characterCode}l`
                case 'italic':
                    return `${characterCode}o`
                case 'underlined':
                    return `${characterCode}n`
                case 'strikethrough':
                    return `${characterCode}m`
                case 'obfuscated':
                    return `${characterCode}k`
            }
        })
        .join('')
}

function formatGradientRuns(
    characters: GradientCharacter[],
    baseColors: string[],
    simplify: boolean,
    formatter: (text: string, colors: string[], formats: TextFormat[]) => string,
): string {
    const output: string[] = []
    let run: GradientCharacter[] = []

    const flush = () => {
        if (!run.length) return
        const text = run.map((character) => character.character).join('')
        const runColors = run
            .map((character) => character.color)
            .filter((color): color is string => color !== null)
        const colors =
            simplify && runColors.length > 1
                ? [runColors[0], runColors[runColors.length - 1]]
                : runColors
        output.push(formatter(text, colors.length ? colors : baseColors, run[0].formats))
        run = []
    }

    for (const character of characters) {
        if (character.newline) {
            flush()
            output.push('\n')
            continue
        }
        if (
            run.length &&
            (run[0].formats.join(',') !== character.formats.join(',') || character.color === null)
        ) {
            flush()
        }
        run.push(character)
    }
    flush()
    return output.join('')
}

function wrapFormats(
    text: string,
    formats: TextFormat[],
    formatMap: Record<TextFormat, readonly [string, string]>,
): string {
    return formats.reduce((result, format) => {
        const [start, end] = formatMap[format]
        return `${start}${result}${end}`
    }, text)
}

function toTextComponents(characters: GradientCharacter[]) {
    return characters.map((character) => {
        const component: Record<string, string | boolean> = { text: character.character }
        if (character.color) component.color = character.color
        for (const format of character.formats) component[format] = true
        return component
    })
}

function toSnbt(characters: GradientCharacter[]): string {
    return `[${characters
        .map((character) => {
            const entries = [`text:${quoteSnbt(character.character)}`]
            if (character.color) entries.push(`color:${quoteSnbt(character.color)}`)
            for (const format of character.formats) entries.push(`${format}:true`)
            return `{${entries.join(',')}}`
        })
        .join(',')}]`
}

function toHtml(characters: GradientCharacter[]): string {
    const lines: string[] = ['']
    for (const character of characters) {
        if (character.newline) {
            lines.push('')
            continue
        }
        const content = character.character === ' ' ? '&nbsp;' : escapeHtml(character.character)
        const colored = character.color
            ? `<span style="color: ${character.color};">${content}</span>`
            : content
        lines[lines.length - 1] += wrapFormats(colored, character.formats, htmlFormats)
    }
    return lines.map((line) => `<p>${line}</p>`).join('')
}

const htmlFormats: Record<TextFormat, readonly [string, string]> = {
    bold: ['<b>', '</b>'],
    italic: ['<i>', '</i>'],
    underlined: ['<u>', '</u>'],
    strikethrough: ['<s>', '</s>'],
    obfuscated: ['', ''],
}

function toCsv(characters: GradientCharacter[]): string {
    const header = 'color,char,bold,italic,underlined,strikethrough,obfuscated'
    const rows = characters.map((character) =>
        [
            character.color ?? '',
            character.newline ? '\\n' : character.character,
            ...TEXT_FORMATS.map((format) => character.formats.includes(format)),
        ]
            .map(csvCell)
            .join(','),
    )
    return [header, ...rows].join('\n')
}

function csvCell(value: string | boolean): string {
    const text = String(value)
    return /[",\n]/.test(text) ? `"${text.replaceAll('"', '""')}"` : text
}

function quoteSnbt(value: string): string {
    return `'${value.replaceAll('\\', '\\\\').replaceAll("'", "\\'")}'`
}

function escapeHtml(value: string): string {
    return value
        .replaceAll('&', '&amp;')
        .replaceAll('<', '&lt;')
        .replaceAll('>', '&gt;')
        .replaceAll('"', '&quot;')
        .replaceAll("'", '&#39;')
}

function hexToColor(hex: string): Color {
    const value = hex.slice(1)
    return {
        red: Number.parseInt(value.slice(0, 2), 16),
        green: Number.parseInt(value.slice(2, 4), 16),
        blue: Number.parseInt(value.slice(4, 6), 16),
    }
}

function colorToHex(color: Color): string {
    return `#${[color.red, color.green, color.blue]
        .map((component) => Math.max(0, Math.min(255, component)).toString(16).padStart(2, '0'))
        .join('')
        .toUpperCase()}`
}

/* ------------------------------------------------------------------------ *
 * Persistence — apps/app-frontend/src/lab/gradient-text/gradient-storage.ts
 * ------------------------------------------------------------------------ */

type GradientPreset = {
    id: string
    name: string
    colors: string[]
    createdAt: string
}

type GradientTextLabState = {
    version: 1
    document: GradientTextDocument
    colors: string[]
    adapterId: GradientFormatId
    vanillaCharacter: '&' | '§'
    simplifyGradients: boolean
    presets: GradientPreset[]
}

const GRADIENT_TEXT_STORAGE_KEY = 'launcher-preview:axolotl.lab.gradient-text.v1'

function createDefaultGradientTextState(): GradientTextLabState {
    return {
        version: 1,
        document: cloneGradientDocument(DEFAULT_GRADIENT_DOCUMENT),
        colors: [...DEFAULT_GRADIENT_COLORS],
        adapterId: 'vanilla',
        vanillaCharacter: '§',
        simplifyGradients: false,
        presets: [],
    }
}

function getBrowserStorage() {
    try {
        return typeof window === 'undefined' ? null : window.localStorage
    } catch {
        return null
    }
}

function loadGradientTextState(): GradientTextLabState {
    const fallback = createDefaultGradientTextState()
    try {
        const raw = getBrowserStorage()?.getItem(GRADIENT_TEXT_STORAGE_KEY)
        if (!raw) return fallback
        return sanitizeGradientTextState(JSON.parse(raw), fallback)
    } catch {
        return fallback
    }
}

function saveGradientTextState(state: GradientTextLabState): void {
    try {
        getBrowserStorage()?.setItem(
            GRADIENT_TEXT_STORAGE_KEY,
            JSON.stringify(sanitizeGradientTextState(state)),
        )
    } catch {
        // Storage may be unavailable (private mode); the page keeps its live state.
    }
}

function sanitizeGradientTextState(
    value: unknown,
    fallback = createDefaultGradientTextState(),
): GradientTextLabState {
    if (!value || typeof value !== 'object') return fallback
    const record = value as Partial<GradientTextLabState>
    const adapterId = gradientFormatAdapters.some((adapter) => adapter.id === record.adapterId)
        ? (record.adapterId as GradientFormatId)
        : fallback.adapterId
    const colors = sanitizeColors(record.colors)
    return {
        version: 1,
        document: record.document ? normalizeGradientDocument(record.document) : fallback.document,
        colors: colors.length ? colors : fallback.colors,
        adapterId,
        vanillaCharacter:
            record.vanillaCharacter === '&' || record.vanillaCharacter === '§'
                ? record.vanillaCharacter
                : fallback.vanillaCharacter,
        simplifyGradients: record.simplifyGradients === true,
        presets: parseGradientPresets(record.presets),
    }
}

function parseGradientPresets(value: unknown): GradientPreset[] {
    if (!Array.isArray(value)) return []
    return value.flatMap((preset, index) => {
        if (!preset || typeof preset !== 'object') return []
        const record = preset as Partial<GradientPreset>
        const colors = sanitizeColors(record.colors)
        if (!colors.length) return []
        return [
            {
                id:
                    typeof record.id === 'string' && record.id
                        ? record.id
                        : `imported-${index}-${Date.now()}`,
                name:
                    typeof record.name === 'string' && record.name.trim()
                        ? record.name.trim().slice(0, 80)
                        : `Preset ${index + 1}`,
                colors,
                createdAt:
                    typeof record.createdAt === 'string' &&
                    !Number.isNaN(Date.parse(record.createdAt))
                        ? record.createdAt
                        : new Date().toISOString(),
            },
        ]
    })
}

function serializeGradientPresets(presets: GradientPreset[]): string {
    return JSON.stringify(
        presets.map(({ name, colors }) => ({ name, colors })),
        null,
        2,
    )
}

function sanitizeColors(value: unknown): string[] {
    if (!Array.isArray(value)) return []
    return value
        .map((color) => (typeof color === 'string' ? normalizeHexColor(color) : null))
        .filter((color): color is string => color !== null)
}

/* ------------------------------------------------------------------------ *
 * Simulated native file dialogs over a virtual file system
 * ------------------------------------------------------------------------ */

type FileDialogMode = 'open' | 'save'

interface VirtualFile {
    name: string
    contents: string | null
}

interface FileDialogState {
    open: boolean
    mode: FileDialogMode
    title: string
    defaultName: string
    extensions: string[]
    files: VirtualFile[]
    selected: string
    name: string
    conflict: boolean
}

/** The folder every file dialog in this preview opens in. */
const HOME_FOLDER = '/Users/axolotl/Downloads'

function createFileDialogState(): FileDialogState {
    return {
        open: false,
        mode: 'open',
        title: '',
        defaultName: '',
        extensions: [],
        files: [],
        selected: '',
        name: '',
        conflict: false,
    }
}

const messages = defineMessages({
    title: { id: 'app.lab.gradient-text.title', defaultMessage: 'Gradient text generator' },
    inputTitle: { id: 'app.lab.gradient-text.input.title', defaultMessage: 'Text' },
    inputPlaceholder: {
        id: 'app.lab.gradient-text.input.placeholder',
        defaultMessage: 'Type your Minecraft text here',
    },
    colorsTitle: { id: 'app.lab.gradient-text.colors.title', defaultMessage: 'Colors' },
    colorStops: { id: 'app.lab.gradient-text.colors.stops', defaultMessage: '{count} color stops' },
    addColor: { id: 'app.lab.gradient-text.colors.add', defaultMessage: 'Add color' },
    randomizeColors: {
        id: 'app.lab.gradient-text.colors.randomize',
        defaultMessage: 'Randomize colors',
    },
    removeColor: { id: 'app.lab.gradient-text.colors.remove', defaultMessage: 'Remove color' },
    moveColorUp: { id: 'app.lab.gradient-text.colors.move-up', defaultMessage: 'Move color up' },
    moveColorDown: {
        id: 'app.lab.gradient-text.colors.move-down',
        defaultMessage: 'Move color down',
    },
    importColors: { id: 'app.lab.gradient-text.colors.import', defaultMessage: 'Import colors' },
    importColorsPlaceholder: {
        id: 'app.lab.gradient-text.colors.import-placeholder',
        defaultMessage: 'Paste HEX, RGB, or a CSS gradient',
    },
    invalidColors: {
        id: 'app.lab.gradient-text.colors.invalid',
        defaultMessage: 'Use at least one valid color.',
    },
    formatTitle: { id: 'app.lab.gradient-text.format.title', defaultMessage: 'Output format' },
    vanillaCharacter: {
        id: 'app.lab.gradient-text.format.vanilla-character',
        defaultMessage: 'Color character',
    },
    simplify: {
        id: 'app.lab.gradient-text.format.simplify',
        defaultMessage: 'Simplify gradient tags',
    },
    previewTitle: { id: 'app.lab.gradient-text.preview.title', defaultMessage: 'Preview' },
    outputTitle: { id: 'app.lab.gradient-text.output.title', defaultMessage: 'Output' },
    copy: { id: 'app.lab.gradient-text.output.copy', defaultMessage: 'Copy output' },
    copied: { id: 'app.lab.gradient-text.output.copied', defaultMessage: 'Output copied' },
    export: { id: 'app.lab.gradient-text.output.export', defaultMessage: 'Export output' },
    presetsTitle: { id: 'app.lab.gradient-text.presets.title', defaultMessage: 'Presets' },
    presetCount: { id: 'app.lab.gradient-text.presets.count', defaultMessage: '{count} saved' },
    presetName: { id: 'app.lab.gradient-text.presets.name', defaultMessage: 'Preset name' },
    savePreset: { id: 'app.lab.gradient-text.presets.save', defaultMessage: 'Save preset' },
    applyPreset: { id: 'app.lab.gradient-text.presets.apply', defaultMessage: 'Apply preset' },
    deletePreset: { id: 'app.lab.gradient-text.presets.delete', defaultMessage: 'Delete preset' },
    importPresets: { id: 'app.lab.gradient-text.presets.import', defaultMessage: 'Import presets' },
    exportPresets: { id: 'app.lab.gradient-text.presets.export', defaultMessage: 'Export presets' },
    downloadTemplate: {
        id: 'app.lab.gradient-text.presets.template',
        defaultMessage: 'Download template',
    },
    presetsImported: {
        id: 'app.lab.gradient-text.presets.imported',
        defaultMessage: 'Imported {count} presets',
    },
    invalidPresetFile: {
        id: 'app.lab.gradient-text.presets.invalid-file',
        defaultMessage: 'This file does not contain valid presets.',
    },
    bold: { id: 'app.lab.gradient-text.format.bold', defaultMessage: 'Bold' },
    italic: { id: 'app.lab.gradient-text.format.italic', defaultMessage: 'Italic' },
    underlined: { id: 'app.lab.gradient-text.format.underlined', defaultMessage: 'Underline' },
    strikethrough: {
        id: 'app.lab.gradient-text.format.strikethrough',
        defaultMessage: 'Strikethrough',
    },
    obfuscated: { id: 'app.lab.gradient-text.format.obfuscated', defaultMessage: 'Obfuscated' },
    adapterVanilla: { id: 'app.lab.gradient-text.adapter.vanilla', defaultMessage: 'Vanilla' },
    adapterVanillaCompatible: {
        id: 'app.lab.gradient-text.adapter.vanilla-compatible',
        defaultMessage: 'Vanilla compatible',
    },
    adapterStandard: {
        id: 'app.lab.gradient-text.adapter.standard',
        defaultMessage: 'Standard HEX',
    },
    adapterCmi: { id: 'app.lab.gradient-text.adapter.cmi', defaultMessage: 'CMI' },
    adapterMiniMessage: {
        id: 'app.lab.gradient-text.adapter.minimessage',
        defaultMessage: 'MiniMessage',
    },
    adapterMiniMessageGradient: {
        id: 'app.lab.gradient-text.adapter.minimessage-gradient',
        defaultMessage: 'MiniMessage gradient',
    },
    adapterMineDown: { id: 'app.lab.gradient-text.adapter.minedown', defaultMessage: 'MineDown' },
    adapterSnbt: { id: 'app.lab.gradient-text.adapter.snbt', defaultMessage: 'Stringified NBT' },
    adapterTrChat: { id: 'app.lab.gradient-text.adapter.trchat', defaultMessage: 'TrChat' },
    adapterTabooLib: { id: 'app.lab.gradient-text.adapter.taboolib', defaultMessage: 'TabooLib' },
    adapterTabooLibGradient: {
        id: 'app.lab.gradient-text.adapter.taboolib-gradient',
        defaultMessage: 'TabooLib gradient',
    },
    adapterRoseGarden: {
        id: 'app.lab.gradient-text.adapter.rosegarden-gradient',
        defaultMessage: 'RoseGarden gradient',
    },
    adapterChatColors: {
        id: 'app.lab.gradient-text.adapter.chat-colors',
        defaultMessage: 'Chat Colors',
    },
    adapterMotd: { id: 'app.lab.gradient-text.adapter.motd', defaultMessage: 'MOTD' },
    adapterBbcode: { id: 'app.lab.gradient-text.adapter.bbcode', defaultMessage: 'BBCode' },
    adapterJson: {
        id: 'app.lab.gradient-text.adapter.json',
        defaultMessage: 'JSON text component',
    },
    adapterHtml: { id: 'app.lab.gradient-text.adapter.html', defaultMessage: 'HTML' },
    adapterCsv: { id: 'app.lab.gradient-text.adapter.csv', defaultMessage: 'CSV' },
    adapterTerraria: { id: 'app.lab.gradient-text.adapter.terraria', defaultMessage: 'Terraria' },
    openDialog: { id: 'launcher-preview.dialog.open', defaultMessage: 'Open' },
    saveDialog: { id: 'launcher-preview.dialog.save', defaultMessage: 'Save' },
    cancelDialog: { id: 'launcher-preview.dialog.cancel', defaultMessage: 'Cancel' },
    fileName: { id: 'launcher-preview.dialog.file-name', defaultMessage: 'File name' },
    saveTo: { id: 'launcher-preview.dialog.save-to', defaultMessage: 'Where: {folder}' },
    emptyFolder: {
        id: 'launcher-preview.dialog.empty-folder',
        defaultMessage: 'No matching files in this folder.',
    },
    replaceFile: {
        id: 'launcher-preview.dialog.replace',
        defaultMessage: 'A file with this name already exists. Saving replaces it.',
    },
})

const adapterMessages: Record<GradientFormatId, (typeof messages)[keyof typeof messages]> = {
    vanilla: messages.adapterVanilla,
    'vanilla-compatible': messages.adapterVanillaCompatible,
    standard: messages.adapterStandard,
    cmi: messages.adapterCmi,
    minimessage: messages.adapterMiniMessage,
    'minimessage-gradient': messages.adapterMiniMessageGradient,
    minedown: messages.adapterMineDown,
    snbt: messages.adapterSnbt,
    trchat: messages.adapterTrChat,
    taboolib: messages.adapterTabooLib,
    'taboolib-gradient': messages.adapterTabooLibGradient,
    'rosegarden-gradient': messages.adapterRoseGarden,
    'chat-colors': messages.adapterChatColors,
    motd: messages.adapterMotd,
    bbcode: messages.adapterBbcode,
    json: messages.adapterJson,
    html: messages.adapterHtml,
    csv: messages.adapterCsv,
    terraria: messages.adapterTerraria,
}

const textFormatIcons: Record<TextFormat, Component> = {
    bold: BoldIcon,
    italic: ItalicIcon,
    underlined: UnderlineIcon,
    strikethrough: StrikethroughIcon,
    obfuscated: AsteriskIcon,
}

/** Stand-in for `assets/lab/minecraft-preview.png` (packaged in the desktop bundle). */
const MINECRAFT_PREVIEW = `url("data:image/svg+xml,${encodeURIComponent(
    "<svg xmlns='http://www.w3.org/2000/svg' shape-rendering='crispEdges' width='64' height='36' viewBox='0 0 64 36'>" +
        "<rect width='64' height='36' fill='#7db3e8'/>" +
        "<rect y='0' width='64' height='6' fill='#8fc0ee'/>" +
        "<rect x='6' y='5' width='9' height='3' fill='#f4f8fc'/><rect x='8' y='4' width='6' height='1' fill='#f4f8fc'/>" +
        "<rect x='38' y='9' width='11' height='3' fill='#eef5fb'/><rect x='41' y='8' width='6' height='1' fill='#eef5fb'/>" +
        "<rect x='22' y='2' width='6' height='2' fill='#f6fafe'/>" +
        "<rect y='19' width='64' height='17' fill='#4f9a3f'/>" +
        "<rect y='19' width='64' height='2' fill='#62ad4c'/>" +
        "<rect x='0' y='26' width='30' height='10' fill='#3f83c9'/><rect x='0' y='26' width='30' height='1' fill='#5f9ddb'/>" +
        "<rect x='34' y='23' width='4' height='4' fill='#5a4632'/><rect x='30' y='18' width='12' height='6' fill='#2f7c33'/>" +
        "<rect x='29' y='21' width='14' height='3' fill='#357f37'/><rect x='33' y='16' width='6' height='3' fill='#3c8a3e'/>" +
        "<rect x='46' y='24' width='3' height='3' fill='#6f7377'/><rect x='49' y='25' width='2' height='2' fill='#7d8185'/>" +
        "<rect x='52' y='21' width='3' height='3' fill='#8b8f93'/><rect x='16' y='28' width='2' height='2' fill='#e4d36a'/>" +
        "<rect x='20' y='31' width='2' height='2' fill='#e4d36a'/><rect x='44' y='33' width='2' height='2' fill='#e4d36a'/>" +
        '</svg>',
)}")`

const state = usePreviewDomain('lab.gradient-text', (preview) => {
    const initialState = loadGradientTextState()
    const textDocument = ref<GradientTextDocument>(initialState.document)
    const colors = ref<string[]>([...initialState.colors])
    const colorInputValues = ref<string[]>([...initialState.colors])
    const adapterId = ref<GradientFormatId>(initialState.adapterId)
    const vanillaCharacter = ref<'&' | '§'>(initialState.vanillaCharacter)
    const simplifyGradients = ref(initialState.simplifyGradients)
    const presets = ref<GradientPreset[]>(initialState.presets)
    const importValue = ref('')
    const importError = ref('')
    const presetName = ref('')
    const obfuscationFrame = ref(0)
    const adapterMenuOpen = ref(false)

    // The real dialogs read a folder on disk; here they read a virtual one that
    // starts with the preset files the app's own guide expects a user to have.
    const files = new Map<string, string>()
    files.set(
        `${HOME_FOLDER}/axolotl-gradient-presets.json`,
        JSON.stringify(
            [
                { name: 'Sunset', colors: ['#FF7A18', '#AF002D', '#FFD200'] },
                { name: 'Ocean', colors: ['#2E3192', '#1BFFFF'] },
                { name: 'Candy', colors: ['#FC466B', '#3F5EFB'] },
            ],
            null,
            2,
        ),
    )
    files.set(
        `${HOME_FOLDER}/axolotl-gradient-presets-template.json`,
        JSON.stringify([{ name: 'Example', colors: DEFAULT_GRADIENT_COLORS }], null, 2),
    )
    files.set(`${HOME_FOLDER}/broken-presets.json`, '{ "presets": [ { "name": "Missing colors" }')

    const dialog = ref<FileDialogState>(createFileDialogState())
    let pending: Array<(path: string | null) => void> = []

    function askOpen(title: string, extensions: string[]): Promise<string | null> {
        return new Promise((resolve) => {
            pending = [resolve]
            const listed = [...files.keys()]
                .filter((path) => path.startsWith(`${HOME_FOLDER}/`))
                .filter((path) => extensions.some((extension) => path.endsWith(`.${extension}`)))
                .map((path) => ({ name: path.slice(HOME_FOLDER.length + 1), contents: null }))
            dialog.value = {
                ...createFileDialogState(),
                open: true,
                mode: 'open',
                title,
                extensions,
                files: listed,
                selected: listed[0]?.name ?? '',
            }
        })
    }

    function askSave(
        title: string,
        defaultName: string,
        extensions: string[],
    ): Promise<string | null> {
        return new Promise((resolve) => {
            pending = [resolve]
            dialog.value = {
                ...createFileDialogState(),
                open: true,
                mode: 'save',
                title,
                defaultName,
                extensions,
                name: defaultName,
            }
        })
    }

    function resolveDialog(path: string | null) {
        const resolvers = pending
        pending = []
        dialog.value = { ...dialog.value, open: false }
        for (const resolve of resolvers) resolve(path)
    }

    function confirmDialog() {
        const current = dialog.value
        if (current.mode === 'open') {
            resolveDialog(current.selected ? `${HOME_FOLDER}/${current.selected}` : null)
            return
        }
        const name = current.name.trim() || current.defaultName
        if (!name) return
        const path = `${HOME_FOLDER}/${name}`
        resolveDialog(path)
    }

    function cancelDialog() {
        resolveDialog(null)
    }

    const dispose = preview.clock.every(() => {
        obfuscationFrame.value += 1
    })

    return {
        value: {
            textDocument,
            colors,
            colorInputValues,
            adapterId,
            vanillaCharacter,
            simplifyGradients,
            presets,
            importValue,
            importError,
            presetName,
            obfuscationFrame,
            adapterMenuOpen,
            files,
            dialog,
            askOpen,
            askSave,
            confirmDialog,
            cancelDialog,
        },
        dispose,
    }
})

const {
    textDocument,
    colors,
    colorInputValues,
    adapterId,
    vanillaCharacter,
    simplifyGradients,
    presets,
    importValue,
    importError,
    presetName,
    obfuscationFrame,
    adapterMenuOpen,
    files,
    dialog,
    askOpen,
    askSave,
    confirmDialog,
    cancelDialog,
} = state

/** Double-clicking a file in the open panel picks it and confirms. */
function confirmFile(name: string) {
    dialog.value.selected = name
    confirmDialog()
}

const { formatMessage } = useVIntl()
const { addNotification } = injectNotificationManager()

const editor: Ref<HTMLDivElement | null> = ref(null)
const newColorPicker: Ref<HTMLInputElement | null> = ref(null)
const activeTextFormats = ref<TextFormat[]>([])
const editorSelection = shallowRef<Range | null>(null)

const OBFUSCATED_PREVIEW_GLYPHS = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789!@#$%^&*'

const currentAdapter = computed(
    () =>
        gradientFormatAdapters.find((adapter) => adapter.id === adapterId.value) ??
        gradientFormatAdapters[0],
)
const output = computed(() =>
    generateGradientOutput(textDocument.value, colors.value, adapterId.value, {
        vanillaCharacter: vanillaCharacter.value,
        simplifyGradients: simplifyGradients.value,
    }),
)
const previewLines = computed(() => {
    const lines: ReturnType<typeof buildGradientCharacters>[] = [[]]
    for (const character of buildGradientCharacters(textDocument.value, colors.value)) {
        if (character.newline) lines.push([])
        else lines[lines.length - 1].push(character)
    }
    return lines
})

const adapterOptions = computed(() =>
    gradientFormatAdapters.map((adapter) => ({
        value: adapter.id,
        label: `${formatAdapterName(adapter.id)} · ${adapter.sample}`,
    })),
)

watch(
    [textDocument, colors, adapterId, vanillaCharacter, simplifyGradients, presets],
    () => {
        saveGradientTextState({
            version: 1,
            document: textDocument.value,
            colors: colors.value,
            adapterId: adapterId.value,
            vanillaCharacter: vanillaCharacter.value,
            simplifyGradients: simplifyGradients.value,
            presets: presets.value,
        })
    },
    { deep: true },
)

onMounted(() => {
    writeDocumentToEditor(textDocument.value)
    document.addEventListener('selectionchange', syncTextFormatState)
})

onUnmounted(() => {
    document.removeEventListener('selectionchange', syncTextFormatState)
})

function onEditorInput() {
    textDocument.value = readDocumentFromEditor()
    syncTextFormatState()
}

function formatText(format: TextFormat) {
    const range = getEditorRange()
    if (!range) return
    restoreEditorSelection(range)

    if (format === 'obfuscated') {
        toggleObfuscatedFormat(range)
        return
    }

    const command =
        format === 'bold'
            ? 'bold'
            : format === 'italic'
              ? 'italic'
              : format === 'underlined'
                ? 'underline'
                : format === 'strikethrough'
                  ? 'strikeThrough'
                  : null
    if (command) document.execCommand(command)
    onEditorInput()
}

function toggleObfuscatedFormat(range: Range) {
    const existingMarker = getObfuscatedMarker(range.startContainer)
    if (existingMarker && existingMarker === getObfuscatedMarker(range.endContainer)) {
        existingMarker.replaceWith(...Array.from(existingMarker.childNodes))
        onEditorInput()
        return
    }

    const marker = document.createElement('span')
    marker.dataset.gradientFormat = 'obfuscated'
    marker.className = 'lab-editor-obfuscated'

    if (range.collapsed) marker.textContent = '\u200B'
    else marker.append(range.extractContents())
    range.insertNode(marker)

    const markerRange = document.createRange()
    if (range.collapsed && marker.firstChild) markerRange.setStart(marker.firstChild, 1)
    else markerRange.selectNodeContents(marker)
    restoreEditorSelection(markerRange)
    onEditorInput()
}

function getEditorRange(): Range | null {
    const root = editor.value
    const selection = window.getSelection()
    if (root && selection?.rangeCount) {
        const range = selection.getRangeAt(0)
        if (root.contains(range.commonAncestorContainer)) return range.cloneRange()
    }
    return editorSelection.value?.cloneRange() ?? null
}

function restoreEditorSelection(range: Range) {
    const selection = window.getSelection()
    if (!selection) return
    selection.removeAllRanges()
    selection.addRange(range)
    editor.value?.focus({ preventScroll: true })
}

function getObfuscatedMarker(node: Node): HTMLElement | null {
    const root = editor.value
    let current = node instanceof HTMLElement ? node : node.parentElement
    while (current && current !== root) {
        if (current.dataset.gradientFormat === 'obfuscated') return current
        current = current.parentElement
    }
    return null
}

function syncTextFormatState() {
    const root = editor.value
    const selection = window.getSelection()
    if (!root || !selection?.rangeCount) return

    const range = selection.getRangeAt(0)
    if (!root.contains(range.commonAncestorContainer)) return
    editorSelection.value = range.cloneRange()
    activeTextFormats.value = TEXT_FORMATS.filter((format) => {
        if (format === 'obfuscated') {
            const startMarker = getObfuscatedMarker(range.startContainer)
            return Boolean(startMarker && startMarker === getObfuscatedMarker(range.endContainer))
        }

        const command =
            format === 'bold'
                ? 'bold'
                : format === 'italic'
                  ? 'italic'
                  : format === 'underlined'
                    ? 'underline'
                    : 'strikeThrough'
        return document.queryCommandState(command)
    })
}

function previewCharacter(character: string, formats: TextFormat[], index: number): string {
    if (!formats.includes('obfuscated') || !character.trim()) return character
    const position = (character.codePointAt(0) ?? 0) + index * 17 + obfuscationFrame.value
    return OBFUSCATED_PREVIEW_GLYPHS[position % OBFUSCATED_PREVIEW_GLYPHS.length]
}

function onEditorPaste(event: ClipboardEvent) {
    event.preventDefault()
    const text = event.clipboardData?.getData('text/plain') ?? ''
    document.execCommand('insertText', false, text)
    onEditorInput()
}

function replaceColors(value: string[]) {
    colors.value = [...value]
    colorInputValues.value = [...value]
}

function updateColor(index: number, value: string) {
    colorInputValues.value[index] = value
    const color = normalizeHexColor(value)
    if (color) colors.value[index] = color
}

function openNewColorPicker() {
    newColorPicker.value?.click()
}

function addPickedColor(value: string) {
    const color = normalizeHexColor(value)
    if (color) replaceColors([...colors.value, color])
}

function randomizeColors() {
    replaceColors(colors.value.map(() => randomGradientColor()))
}

function removeColor(index: number) {
    if (colors.value.length <= 1) return
    const nextColors = [...colors.value]
    nextColors.splice(index, 1)
    replaceColors(nextColors)
}

function moveColor(index: number, direction: -1 | 1) {
    const destination = index + direction
    if (destination < 0 || destination >= colors.value.length) return
    const nextColors = [...colors.value]
    const [color] = nextColors.splice(index, 1)
    nextColors.splice(destination, 0, color)
    replaceColors(nextColors)
}

function applyImportedColors() {
    const parsed = parseGradientColors(importValue.value)
    if (!parsed.length) {
        importError.value = formatMessage(messages.invalidColors)
        return
    }
    replaceColors(parsed)
    importValue.value = ''
    importError.value = ''
}

function savePreset() {
    const name = presetName.value.trim()
    if (!name) return
    presets.value.push({
        id: crypto.randomUUID(),
        name: name.slice(0, 80),
        colors: [...colors.value],
        createdAt: new Date().toISOString(),
    })
    presetName.value = ''
}

function applyPreset(preset: GradientPreset) {
    replaceColors(preset.colors)
}

function deletePreset(id: string) {
    presets.value = presets.value.filter((preset) => preset.id !== id)
}

async function copyOutput() {
    try {
        await navigator.clipboard.writeText(output.value)
    } catch {
        // The Tauri clipboard command cannot fail this way in the desktop app.
    }
    addNotification({ type: 'success', title: formatMessage(messages.copied) })
}

async function exportOutput() {
    const adapter = currentAdapter.value
    const path = await askSave(
        formatMessage(messages.export),
        `minecraft-gradient.${adapter.extension}`,
        [adapter.extension],
    )
    if (!path) return
    files.set(path, output.value)
}

async function importPresets() {
    const path = await askOpen(formatMessage(messages.importPresets), ['json'])
    if (!path) return
    try {
        const raw = files.get(path) ?? ''
        const imported = parseGradientPresets(JSON.parse(raw))
        if (!imported.length) {
            addNotification({ type: 'error', title: formatMessage(messages.invalidPresetFile) })
            return
        }
        presets.value = [...presets.value, ...imported]
        addNotification({
            type: 'success',
            title: formatMessage(messages.presetsImported, { count: imported.length }),
        })
    } catch (error) {
        addNotification({
            type: 'error',
            title: errorTitle(),
            text: error instanceof Error ? error.message : String(error),
        })
    }
}

async function exportPresets(template = false) {
    const path = await askSave(
        template ? formatMessage(messages.downloadTemplate) : formatMessage(messages.exportPresets),
        template ? 'axolotl-gradient-presets-template.json' : 'axolotl-gradient-presets.json',
        ['json'],
    )
    if (!path) return
    const exportValue = template
        ? [{ name: 'Example', colors: DEFAULT_GRADIENT_COLORS }]
        : presets.value
    files.set(path, serializeGradientPresets(exportValue as GradientPreset[]))
}

function errorTitle() {
    return formatMessage({ id: 'app.error.generic-title', defaultMessage: 'An error occurred' })
}

function readDocumentFromEditor(): GradientTextDocument {
    const root = editor.value
    if (!root) return textDocument.value
    const lines: GradientTextRun[][] = [[]]
    let line = lines[0]

    const appendText = (text: string, formats: TextFormat[]) => {
        const visibleText = text.replaceAll('\u200B', '')
        if (!visibleText) return
        const previous = line[line.length - 1]
        if (previous && previous.formats.join(',') === formats.join(','))
            previous.text += visibleText
        else line.push({ text: visibleText, formats: [...formats] })
    }
    const newLine = () => {
        line = []
        lines.push(line)
    }
    const walk = (node: Node, formats: TextFormat[]) => {
        if (node.nodeType === Node.TEXT_NODE) {
            appendText(node.textContent ?? '', formats)
            return
        }
        if (!(node instanceof HTMLElement)) return
        if (node.tagName === 'BR') {
            newLine()
            return
        }
        const nextFormats = [...formats]
        const tag = node.tagName
        if ((tag === 'B' || tag === 'STRONG') && !nextFormats.includes('bold'))
            nextFormats.push('bold')
        if ((tag === 'I' || tag === 'EM') && !nextFormats.includes('italic'))
            nextFormats.push('italic')
        if (tag === 'U' && !nextFormats.includes('underlined')) nextFormats.push('underlined')
        if (
            (tag === 'S' || tag === 'STRIKE' || tag === 'DEL') &&
            !nextFormats.includes('strikethrough')
        ) {
            nextFormats.push('strikethrough')
        }
        if (
            (tag === 'CODE' || node.dataset.gradientFormat === 'obfuscated') &&
            !nextFormats.includes('obfuscated')
        ) {
            nextFormats.push('obfuscated')
        }
        const isBlock = tag === 'DIV' || tag === 'P'
        if (isBlock && line.length) newLine()
        for (const child of Array.from(node.childNodes)) walk(child, nextFormats)
    }

    for (const child of Array.from(root.childNodes)) walk(child, [])
    return normalizeGradientDocument({ lines })
}

function writeDocumentToEditor(value: GradientTextDocument) {
    if (!editor.value) return
    editor.value.replaceChildren()
    for (const sourceLine of normalizeGradientDocument(value).lines) {
        const line = document.createElement('div')
        if (!sourceLine.length || !sourceLine.some((run) => run.text))
            line.append(document.createElement('br'))
        for (const run of sourceLine) {
            const span = document.createElement('span')
            span.textContent = run.text
            if (run.formats.includes('bold')) span.style.fontWeight = '700'
            if (run.formats.includes('italic')) span.style.fontStyle = 'italic'
            if (run.formats.includes('underlined')) span.style.textDecoration = 'underline'
            if (run.formats.includes('strikethrough')) span.style.textDecoration = 'line-through'
            if (run.formats.includes('obfuscated')) {
                span.dataset.gradientFormat = 'obfuscated'
                span.className = 'lab-editor-obfuscated'
            }
            line.append(span)
        }
        editor.value.append(line)
    }
}

function formatLabel(format: TextFormat): string {
    return formatMessage(messages[format])
}

function formatAdapterName(id: GradientFormatId): string {
    return formatMessage(adapterMessages[id])
}

/** Adapter picker: an in-flow menu, because the shared dropdown is viewport-fixed. */
function selectAdapter(value: string) {
    adapterId.value = value as GradientFormatId
    adapterMenuOpen.value = false
}

function onAdapterKeydown(event: KeyboardEvent) {
    if (event.key === 'Enter' || event.key === ' ') {
        event.preventDefault()
        adapterMenuOpen.value = !adapterMenuOpen.value
    } else if (event.key === 'Escape') {
        adapterMenuOpen.value = false
    }
}
</script>

<template>
    <main class="mx-auto flex w-full max-w-[90rem] flex-col gap-5 p-6">
        <header class="flex min-w-0 flex-wrap items-center justify-between gap-4">
            <h1 class="m-0 min-w-0 truncate text-2xl font-bold text-[var(--color-text-primary)]">
                {{ formatMessage(messages.title) }}
            </h1>
            <div class="flex min-w-0 flex-1 flex-wrap items-center justify-end gap-2">
                <div class="lab-format-control">
                    <span>{{ formatMessage(messages.formatTitle) }}</span>
                    <div class="adapter-combobox relative min-w-0" @keydown="onAdapterKeydown">
                        <div
                            role="button"
                            tabindex="0"
                            class="relative flex min-h-5 w-full cursor-pointer items-center justify-between gap-2.5 overflow-hidden rounded-xl bg-surface-4 px-4 py-2 text-left text-button-text"
                            :aria-expanded="adapterMenuOpen"
                            aria-haspopup="listbox"
                            @click="adapterMenuOpen = !adapterMenuOpen"
                        >
                            <span
                                class="min-w-0 truncate font-semibold leading-tight text-[var(--color-text-default)]"
                            >
                                {{ formatAdapterName(adapterId) }} · {{ currentAdapter.sample }}
                            </span>
                            <ChevronRightIcon
                                class="size-5 shrink-0 -rotate-90 text-[var(--color-text-tertiary)] transition-transform duration-150"
                                :class="{ 'rotate-90': adapterMenuOpen }"
                            />
                        </div>
                        <Transition
                            enter-active-class="transition-opacity duration-150"
                            leave-active-class="transition-opacity duration-150"
                            enter-from-class="opacity-0"
                            leave-to-class="opacity-0"
                        >
                            <div
                                v-if="adapterMenuOpen"
                                role="listbox"
                                class="absolute right-0 top-[calc(100%+8px)] z-40 flex max-h-[300px] w-full min-w-[21rem] flex-col overflow-y-auto overscroll-contain rounded-[14px] border border-solid border-surface-5 bg-surface-4 shadow-2xl"
                            >
                                <button
                                    v-for="option in adapterOptions"
                                    :key="option.value"
                                    type="button"
                                    role="option"
                                    :aria-selected="adapterId === option.value"
                                    class="flex w-full cursor-pointer items-center border-0 px-4 py-3 text-left font-semibold"
                                    :class="
                                        adapterId === option.value
                                            ? 'bg-brand-highlight text-brand'
                                            : 'bg-transparent text-[var(--color-text-default)]'
                                    "
                                    @click="selectAdapter(option.value)"
                                >
                                    {{ option.label }}
                                </button>
                            </div>
                        </Transition>
                    </div>
                </div>
                <Button type="outlined" size="md" @click="copyOutput"
                    ><ClipboardCopyIcon />{{ formatMessage(messages.copy) }}
                </Button>
                <Button type="colored" color="brand" size="md" @click="exportOutput"
                    ><DownloadIcon />{{ formatMessage(messages.export) }}
                </Button>
            </div>
        </header>

        <div class="lab-workbench" data-onboarding-id="lab-gradient-text-editor">
            <section class="lab-panel min-w-0">
                <section class="lab-panel-section p-4">
                    <div class="mb-3 flex flex-wrap items-center justify-between gap-3">
                        <h2 class="m-0 text-base font-bold text-[var(--color-text-primary)]">
                            {{ formatMessage(messages.inputTitle) }}
                        </h2>
                        <div
                            class="flex items-center gap-1"
                            role="toolbar"
                            :aria-label="formatMessage(messages.inputTitle)"
                        >
                            <Button
                                v-for="format in TEXT_FORMATS"
                                :key="format"
                                circular
                                icon-only
                                size="2xs"
                                type="quiet"
                                :title="formatLabel(format)"
                                :label="formatLabel(format)"
                                @mousedown.prevent
                                @click="formatText(format)"
                            >
                                <component :is="textFormatIcons[format]" />
                            </Button>
                        </div>
                    </div>
                    <div
                        ref="editor"
                        contenteditable="true"
                        role="textbox"
                        aria-multiline="true"
                        :aria-label="formatMessage(messages.inputTitle)"
                        :data-placeholder="formatMessage(messages.inputPlaceholder)"
                        class="lab-editor max-h-80 min-h-44 overflow-y-auto rounded-lg bg-surface-4 px-3 py-2.5 text-base leading-6 text-[var(--color-text-primary)] outline-none transition-shadow focus:ring-4 focus:ring-brand-shadow"
                        @input="onEditorInput"
                        @focus="syncTextFormatState"
                        @keyup="syncTextFormatState"
                        @mouseup="syncTextFormatState"
                        @paste="onEditorPaste"
                    ></div>
                </section>

                <section class="lab-panel-section p-4">
                    <div class="mb-3 flex flex-wrap items-center justify-between gap-2">
                        <div>
                            <h2 class="m-0 text-base font-bold text-[var(--color-text-primary)]">
                                {{ formatMessage(messages.colorsTitle) }}
                            </h2>
                            <p
                                class="m-0 mt-0.5 text-xs font-medium text-[var(--color-text-tertiary)]"
                            >
                                {{ formatMessage(messages.colorStops, { count: colors.length }) }}
                            </p>
                        </div>
                        <div class="flex items-center gap-1">
                            <Button
                                type="quiet"
                                size="2xs"
                                circular
                                icon-only
                                :title="formatMessage(messages.randomizeColors)"
                                :label="formatMessage(messages.randomizeColors)"
                                @click="randomizeColors"
                                ><RefreshCwIcon />
                            </Button>
                            <input
                                ref="newColorPicker"
                                type="color"
                                :value="colors[colors.length - 1]"
                                :aria-label="formatMessage(messages.addColor)"
                                class="lab-new-color-picker"
                                @change="addPickedColor(($event.target as HTMLInputElement).value)"
                            />
                            <Button
                                type="colored"
                                color="brand"
                                size="2xs"
                                @click="openNewColorPicker"
                                ><PlusIcon />{{ formatMessage(messages.addColor) }}
                            </Button>
                        </div>
                    </div>
                    <div
                        class="lab-color-rail"
                        :style="{ background: `linear-gradient(90deg, ${colors.join(', ')})` }"
                    ></div>
                    <div class="lab-color-list">
                        <div v-for="(color, index) in colors" :key="index" class="lab-color-row">
                            <label class="lab-color-swatch" :style="{ backgroundColor: color }">
                                <input
                                    class="lab-color-swatch-input"
                                    type="color"
                                    :value="color"
                                    :aria-label="`${formatMessage(messages.colorsTitle)} ${index + 1}`"
                                    @input="
                                        updateColor(
                                            index,
                                            ($event.target as HTMLInputElement).value,
                                        )
                                    "
                                />
                            </label>
                            <StyledInput
                                v-model="colorInputValues[index]"
                                size="small"
                                input-class="font-mono"
                                :input-attrs="{ 'aria-label': color }"
                                @update:model-value="updateColor(index, String($event))"
                            />
                            <div class="flex items-center gap-1">
                                <Button
                                    type="quiet"
                                    size="2xs"
                                    circular
                                    icon-only
                                    :title="formatMessage(messages.moveColorUp)"
                                    :label="formatMessage(messages.moveColorUp)"
                                    :disabled="index === 0"
                                    @click="moveColor(index, -1)"
                                    ><ArrowUpIcon />
                                </Button>
                                <Button
                                    type="quiet"
                                    size="2xs"
                                    circular
                                    icon-only
                                    :title="formatMessage(messages.moveColorDown)"
                                    :label="formatMessage(messages.moveColorDown)"
                                    :disabled="index === colors.length - 1"
                                    @click="moveColor(index, 1)"
                                    ><ArrowDownIcon />
                                </Button>
                                <Button
                                    type="quiet"
                                    color="red"
                                    size="2xs"
                                    circular
                                    icon-only
                                    :title="formatMessage(messages.removeColor)"
                                    :label="formatMessage(messages.removeColor)"
                                    :disabled="colors.length <= 1"
                                    @click="removeColor(index)"
                                    ><TrashIcon />
                                </Button>
                            </div>
                        </div>
                    </div>
                    <div class="mt-3 flex gap-2">
                        <StyledInput
                            v-model="importValue"
                            :placeholder="formatMessage(messages.importColorsPlaceholder)"
                            wrapper-class="min-w-0 flex-1"
                            @keydown.enter.prevent="applyImportedColors"
                        />
                        <Button type="outlined" size="md" @click="applyImportedColors">{{
                            formatMessage(messages.importColors)
                        }}</Button>
                    </div>
                    <p v-if="importError" class="m-0 mt-2 text-sm text-red">{{ importError }}</p>
                </section>

                <Accordion
                    class="lab-panel-section p-4"
                    content-class="pt-3"
                    button-class="group flex w-full items-center gap-2 border-0 bg-transparent p-0 text-left"
                >
                    <template #title>
                        <span class="text-base font-bold text-[var(--color-text-primary)]">{{
                            formatMessage(messages.presetsTitle)
                        }}</span>
                        <span
                            v-if="presets.length"
                            class="text-sm font-medium text-[var(--color-text-tertiary)]"
                            >{{
                                formatMessage(messages.presetCount, { count: presets.length })
                            }}</span
                        >
                    </template>
                    <div class="flex flex-col gap-3">
                        <div class="flex gap-2">
                            <StyledInput
                                v-model="presetName"
                                :placeholder="formatMessage(messages.presetName)"
                                wrapper-class="min-w-0 flex-1"
                                @keydown.enter.prevent="savePreset"
                            />
                            <Button
                                type="colored"
                                color="brand"
                                size="2xs"
                                :disabled="!presetName.trim()"
                                @click="savePreset"
                                ><PlusIcon />{{ formatMessage(messages.savePreset) }}
                            </Button>
                        </div>
                        <div v-if="presets.length" class="flex flex-col gap-1.5">
                            <div
                                v-for="preset in presets"
                                :key="preset.id"
                                class="flex items-center gap-2 rounded-lg bg-surface-4 p-2"
                            >
                                <button
                                    class="min-w-0 flex-1 cursor-pointer border-0 bg-transparent p-0 text-left"
                                    :aria-label="formatMessage(messages.applyPreset)"
                                    @click="applyPreset(preset)"
                                >
                                    <span
                                        class="block truncate text-sm font-semibold text-[var(--color-text-primary)]"
                                        >{{ preset.name }}</span
                                    >
                                    <span class="mt-1.5 flex h-1.5 overflow-hidden rounded-full">
                                        <span
                                            v-for="(presetColor, colorIndex) in preset.colors"
                                            :key="`${preset.id}-${colorIndex}`"
                                            class="flex-1"
                                            :style="{ backgroundColor: presetColor }"
                                        ></span>
                                    </span>
                                </button>
                                <Button
                                    type="quiet"
                                    color="red"
                                    size="2xs"
                                    circular
                                    icon-only
                                    :title="formatMessage(messages.deletePreset)"
                                    :label="formatMessage(messages.deletePreset)"
                                    @click="deletePreset(preset.id)"
                                    ><TrashIcon />
                                </Button>
                            </div>
                        </div>
                        <div class="flex flex-wrap gap-2">
                            <Button type="outlined" size="2xs" @click="importPresets"
                                ><UploadIcon />{{ formatMessage(messages.importPresets) }}
                            </Button>
                            <Button type="outlined" size="2xs" @click="exportPresets()"
                                ><DownloadIcon />{{ formatMessage(messages.exportPresets) }}
                            </Button>
                            <Button type="outlined" size="2xs" @click="exportPresets(true)"
                                ><DownloadIcon />{{ formatMessage(messages.downloadTemplate) }}
                            </Button>
                        </div>
                    </div>
                </Accordion>
            </section>

            <section class="lab-panel min-w-0">
                <div
                    v-if="
                        currentAdapter.supportsVanillaCharacter || currentAdapter.supportsSimplify
                    "
                    class="lab-panel-section flex flex-wrap items-center gap-x-5 gap-y-3 p-4"
                >
                    <div
                        v-if="currentAdapter.supportsVanillaCharacter"
                        class="flex items-center gap-2"
                    >
                        <span class="text-sm font-semibold text-[var(--color-text-default)]">{{
                            formatMessage(messages.vanillaCharacter)
                        }}</span>
                        <div
                            class="lab-segmented"
                            role="group"
                            :aria-label="formatMessage(messages.vanillaCharacter)"
                        >
                            <button
                                class="lab-segment"
                                :class="{ active: vanillaCharacter === '&' }"
                                @click="vanillaCharacter = '&'"
                            >
                                &amp;
                            </button>
                            <button
                                class="lab-segment"
                                :class="{ active: vanillaCharacter === '§' }"
                                @click="vanillaCharacter = '§'"
                            >
                                §
                            </button>
                        </div>
                    </div>
                    <label
                        v-if="currentAdapter.supportsSimplify"
                        class="flex cursor-pointer items-center gap-2 text-sm font-semibold text-[var(--color-text-default)]"
                    >
                        <input
                            v-model="simplifyGradients"
                            type="checkbox"
                            class="size-4 accent-[--color-brand]"
                        />
                        {{ formatMessage(messages.simplify) }}
                    </label>
                </div>

                <section class="lab-panel-section p-4">
                    <div class="mb-3 flex items-center justify-between gap-3">
                        <h2 class="m-0 text-base font-bold text-[var(--color-text-primary)]">
                            {{ formatMessage(messages.previewTitle) }}
                        </h2>
                        <span class="font-mono text-xs text-[var(--color-text-tertiary)]">{{
                            currentAdapter.sample
                        }}</span>
                    </div>
                    <div
                        class="minecraft-preview-box"
                        :style="{ backgroundImage: MINECRAFT_PREVIEW }"
                    >
                        <div class="minecraft-preview-content">
                            <p v-for="(line, lineIndex) in previewLines" :key="lineIndex">
                                <span
                                    v-for="(character, characterIndex) in line"
                                    :key="characterIndex"
                                    :class="{
                                        'is-bold': character.formats.includes('bold'),
                                        'is-italic': character.formats.includes('italic'),
                                        'is-underlined': character.formats.includes('underlined'),
                                        'is-strikethrough':
                                            character.formats.includes('strikethrough'),
                                        'is-obfuscated': character.formats.includes('obfuscated'),
                                        'is-space': character.character.trim() === '',
                                    }"
                                    :style="{
                                        '--minecraft-text-color': character.color ?? 'inherit',
                                        '--minecraft-text-shadow-color': getMinecraftTextShadow(
                                            character.color,
                                        ),
                                    }"
                                    >{{
                                        character.character.trim() === ''
                                            ? '\u00A0'
                                            : previewCharacter(
                                                  character.character,
                                                  character.formats,
                                                  characterIndex,
                                              )
                                    }}</span
                                >
                            </p>
                        </div>
                    </div>
                </section>

                <section class="lab-panel-section p-4">
                    <div class="mb-3 flex items-center justify-between gap-3">
                        <h2 class="m-0 text-base font-bold text-[var(--color-text-primary)]">
                            {{ formatMessage(messages.outputTitle) }}
                        </h2>
                        <span class="text-sm font-medium text-[var(--color-text-tertiary)]">{{
                            formatAdapterName(currentAdapter.id)
                        }}</span>
                    </div>
                    <textarea
                        readonly
                        :value="output"
                        class="min-h-52 w-full resize-y rounded-lg bg-surface-4 p-3 font-mono text-sm leading-6 text-[var(--color-text-primary)] outline-none transition-shadow focus:ring-4 focus:ring-brand-shadow"
                        :aria-label="formatMessage(messages.outputTitle)"
                    ></textarea>
                </section>
            </section>
        </div>

        <PreviewModal
            :header="dialog.title"
            :closable="true"
            :close-on-click-outside="true"
            :on-hide="cancelDialog"
            max-width="34rem"
        >
            <div class="flex flex-col gap-4">
                <p class="m-0 text-sm text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.saveTo, { folder: HOME_FOLDER }) }}
                </p>
                <div v-if="dialog.mode === 'open'" class="file-list">
                    <button
                        v-for="file in dialog.files"
                        :key="file.name"
                        type="button"
                        class="file-row"
                        :class="{ 'file-row--active': dialog.selected === file.name }"
                        @dblclick="confirmFile(file.name)"
                        @click="dialog.selected = file.name"
                    >
                        {{ file.name }}
                    </button>
                    <p
                        v-if="!dialog.files.length"
                        class="m-0 text-sm text-[var(--color-text-tertiary)]"
                    >
                        {{ formatMessage(messages.emptyFolder) }}
                    </p>
                </div>
                <label
                    v-else
                    class="flex flex-col gap-2 text-sm font-semibold text-[var(--color-text-primary)]"
                >
                    {{ formatMessage(messages.fileName) }}
                    <StyledInput v-model="dialog.name" :placeholder="dialog.defaultName" />
                </label>
                <p v-if="dialog.conflict" class="m-0 text-sm text-red">
                    {{ formatMessage(messages.replaceFile) }}
                </p>
            </div>
            <template #actions>
                <div class="flex justify-end gap-2">
                    <Button @click="cancelDialog">{{
                        formatMessage(messages.cancelDialog)
                    }}</Button>
                    <Button
                        type="colored"
                        color="brand"
                        :disabled="dialog.mode === 'open' && !dialog.selected"
                        @click="confirmDialog"
                    >
                        {{
                            dialog.mode === 'open'
                                ? formatMessage(messages.openDialog)
                                : formatMessage(messages.saveDialog)
                        }}
                    </Button>
                </div>
            </template>
        </PreviewModal>
    </main>
</template>

<style scoped>
/*
 * Ported from apps/app-frontend/src/pages/LabGradientText.vue without the
 * responsive branches that cannot fire at the 1280px window, and without the
 * `@font-face` whose `url(v-bind())` never loads (the preview text falls back
 * to monospace there, and to Courier here).
 */
.lab-workbench {
    display: grid;
    grid-template-columns: minmax(19rem, 0.82fr) minmax(28rem, 1.18fr);
    align-items: start;
    gap: 1.25rem;
}

.lab-panel {
    overflow: hidden;
    border: 1px solid var(--color-surface-5);
    border-radius: var(--radius-lg);
    background: var(--color-surface-2);
}

.lab-panel-section + .lab-panel-section {
    border-top: 1px solid var(--color-surface-5);
}

.lab-editor:empty::before {
    color: var(--color-text-tertiary);
    content: attr(data-placeholder);
    pointer-events: none;
}

.lab-editor-obfuscated {
    font-family: monospace;
    letter-spacing: 0.05em;
}

.lab-format-control {
    display: flex;
    min-width: 0;
    align-items: center;
    gap: 0.625rem;
    color: var(--color-text-default);
    font-size: 0.875rem;
    font-weight: 700;
}

.lab-format-control .adapter-combobox {
    width: min(21rem, 640px);
}

.lab-color-rail {
    height: 0.5rem;
    margin-bottom: 0.5rem;
    border: 1px solid var(--color-surface-5);
    border-radius: 0.25rem;
}

.lab-color-list {
    max-height: 13rem;
    overflow-y: auto;
}

.lab-color-row {
    display: grid;
    grid-template-columns: 2.5rem minmax(0, 1fr) auto;
    align-items: center;
    gap: 0.5rem;
    padding: 0.5rem 0.125rem;
}

.lab-color-row + .lab-color-row {
    border-top: 1px solid var(--color-surface-4);
}

.lab-color-swatch {
    position: relative;
    display: block;
    width: 2rem;
    height: 2rem;
    margin-inline: auto;
    overflow: hidden;
    border: 2px solid var(--color-surface-5);
    border-radius: 0.375rem;
    box-shadow: inset 0 0 0 1px rgb(255 255 255 / 12%);
    cursor: pointer;
}

.lab-color-swatch:focus-within {
    border-color: var(--color-brand);
    outline: 2px solid var(--color-brand-shadow);
    outline-offset: 1px;
}

.lab-color-swatch-input,
.lab-new-color-picker {
    appearance: auto !important;
    -webkit-appearance: auto !important;
}

.lab-color-swatch-input {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    opacity: 0;
    cursor: pointer;
}

.lab-new-color-picker {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    opacity: 0;
    pointer-events: none;
}

.lab-segmented {
    display: inline-flex;
    padding: 0.125rem;
    border-radius: 0.375rem;
    background: var(--color-surface-4);
}

.lab-segment {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-width: 1.75rem;
    height: 1.75rem;
    padding: 0 0.375rem;
    border: 0;
    border-radius: 0.25rem;
    background: transparent;
    color: var(--color-text-tertiary);
    cursor: pointer;
    font-family: monospace;
    font-weight: 700;
}

.lab-segment:hover,
.lab-segment.active {
    background: var(--surface-4);
    color: var(--color-text-primary);
}

.lab-segment:focus-visible {
    outline: 2px solid var(--color-brand);
    outline-offset: 1px;
}

.minecraft-preview-box {
    width: min(100%, 44rem);
    min-height: 16rem;
    max-height: 26rem;
    margin-inline: auto;
    overflow-x: hidden;
    overflow-y: auto;
    padding: 1.25rem;
    background-position: center bottom;
    background-repeat: no-repeat;
    background-size: cover;
    image-rendering: pixelated;
    font-family: Courier, 'Courier New', monospace;
    font-size: 18px;
    line-height: 24px;
    word-break: break-all;
    word-wrap: break-word;
}

.minecraft-preview-content {
    min-height: 2.375rem;
    padding: 0.5rem;
    background-color: rgb(1 1 1 / 40%);
}

.minecraft-preview-content p {
    min-height: 1.5rem;
    margin: 0;
}

.minecraft-preview-content span {
    color: var(--minecraft-text-color);
    text-shadow: 0.125em 0.125em var(--minecraft-text-shadow-color);
}

.minecraft-preview-content .is-bold {
    font-weight: 700;
}

.minecraft-preview-content .is-italic {
    font-style: italic;
}

.minecraft-preview-content .is-underlined,
.minecraft-preview-content .is-strikethrough {
    position: relative;
    display: inline-block;
}

.minecraft-preview-content .is-underlined::after,
.minecraft-preview-content .is-strikethrough::before {
    position: absolute;
    left: 0;
    display: inline-block;
    width: 100%;
    height: 2px;
    background: var(--minecraft-text-color);
    box-shadow: 0.125em 0.125em var(--minecraft-text-shadow-color);
    content: '';
}

.minecraft-preview-content .is-underlined::after {
    bottom: -2px;
}

.minecraft-preview-content .is-strikethrough::before {
    top: calc(50% - 2px);
}

.minecraft-preview-content .is-obfuscated {
    font-family: monospace;
    letter-spacing: 0.08em;
}

.minecraft-preview-content .is-space {
    font-family: Arial, sans-serif;
}

.file-list {
    display: flex;
    max-height: 16rem;
    flex-direction: column;
    gap: 0.25rem;
    overflow-y: auto;
    border: 1px solid var(--surface-5);
    border-radius: var(--radius-md);
    background: var(--surface-2);
    padding: 0.5rem;
}

.file-row {
    border: 0;
    border-radius: var(--radius-sm);
    background: transparent;
    padding: 0.45rem 0.6rem;
    color: var(--color-text-default);
    text-align: left;
    cursor: pointer;
}

.file-row:hover {
    background: var(--surface-4);
}

.file-row--active {
    background: var(--color-brand-highlight);
    color: var(--color-brand);
}
</style>
