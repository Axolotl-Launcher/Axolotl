<script setup lang="ts">
import {
    ArrowDownIcon,
    ArrowUpIcon,
    AsteriskIcon,
    BoldIcon,
    ClipboardCopyIcon,
    DownloadIcon,
    ItalicIcon,
    PlusIcon,
    RefreshCwIcon,
    StrikethroughIcon,
    TrashIcon,
    UnderlineIcon,
    UploadIcon,
} from '@modrinth/assets'
import {
    Accordion,
    Button,
    Combobox,
    defineMessages,
    injectNotificationManager,
    StyledInput,
    useVIntl,
} from '@modrinth/ui'
import { open, save } from '@tauri-apps/plugin-dialog'
import { readTextFile, writeTextFile } from '@tauri-apps/plugin-fs'
import {
    type Component,
    computed,
    onMounted,
    onUnmounted,
    ref,
    shallowRef,
    useTemplateRef,
    watch,
} from 'vue'

import minecraftPreviewFont from '@/assets/lab/Minecraft_AE.ttf'
import minecraftPreviewBackground from '@/assets/lab/minecraft-preview.png'
import {
    type GradientPreset,
    loadGradientTextState,
    parseGradientPresets,
    saveGradientTextState,
    serializeGradientPresets,
} from '@/lab/gradient-text/gradient-storage'
import {
    buildGradientCharacters,
    DEFAULT_GRADIENT_COLORS,
    generateGradientOutput,
    getMinecraftTextShadow,
    type GradientFormatAdapter,
    gradientFormatAdapters,
    type GradientFormatId,
    type GradientTextDocument,
    type GradientTextRun,
    normalizeGradientDocument,
    normalizeHexColor,
    parseGradientColors,
    randomGradientColor,
    TEXT_FORMATS,
    type TextFormat,
} from '@/lab/gradient-text/gradient-text'

const { addNotification, handleError } = injectNotificationManager()
const { formatMessage } = useVIntl()
const editor = useTemplateRef<HTMLDivElement>('editor')
const newColorPicker = useTemplateRef<HTMLInputElement>('newColorPicker')
const activeTextFormats = ref<TextFormat[]>([])
const editorSelection = shallowRef<Range | null>(null)
const obfuscationFrame = ref(0)
const initialState = loadGradientTextState()
const textDocument = ref<GradientTextDocument>(initialState.document)
const colors = ref([...initialState.colors])
const colorInputValues = ref([...initialState.colors])
const adapterId = ref<GradientFormatId>(initialState.adapterId)
const vanillaCharacter = ref<'&' | '§'>(initialState.vanillaCharacter)
const simplifyGradients = ref(initialState.simplifyGradients)
const presets = ref<GradientPreset[]>(initialState.presets)
const importValue = ref('')
const importError = ref('')
const presetName = ref('')
let obfuscationTimer: ReturnType<typeof window.setInterval> | undefined

const OBFUSCATED_PREVIEW_GLYPHS = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789!@#$%^&*'

const messages = defineMessages({
    title: { id: 'app.lab.gradient-text.title', defaultMessage: 'Gradient text generator' },
    inputTitle: { id: 'app.lab.gradient-text.input.title', defaultMessage: 'Text' },
    inputPlaceholder: {
        id: 'app.lab.gradient-text.input.placeholder',
        defaultMessage: 'Type your Minecraft text here',
    },
    presetExample: {
        id: 'app.lab.gradient-text.preset.example',
        defaultMessage: 'Example',
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
    obfuscationTimer = window.setInterval(() => {
        obfuscationFrame.value += 1
    }, 120)
})

onUnmounted(() => {
    document.removeEventListener('selectionchange', syncTextFormatState)
    if (obfuscationTimer) window.clearInterval(obfuscationTimer)
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
        addNotification({ type: 'success', title: formatMessage(messages.copied) })
    } catch (error) {
        handleError(error)
    }
}

async function exportOutput() {
    try {
        const path = await save({
            defaultPath: `minecraft-gradient.${currentAdapter.value.extension}`,
            filters: [
                { name: currentAdapter.value.label, extensions: [currentAdapter.value.extension] },
            ],
        })
        if (path) await writeTextFile(path, output.value)
    } catch (error) {
        handleError(error)
    }
}

async function importPresets() {
    try {
        const path = await open({
            multiple: false,
            filters: [{ name: 'JSON', extensions: ['json'] }],
        })
        if (typeof path !== 'string') return
        const imported = parseGradientPresets(JSON.parse(await readTextFile(path)))
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
        handleError(error)
    }
}

async function exportPresets(template = false) {
    try {
        const path = await save({
            defaultPath: template
                ? 'axolotl-gradient-presets-template.json'
                : 'axolotl-gradient-presets.json',
            filters: [{ name: 'JSON', extensions: ['json'] }],
        })
        if (!path) return
        const exportValue = template
            ? [
                  {
                      name: formatMessage(messages.presetExample),
                      colors: DEFAULT_GRADIENT_COLORS,
                  },
              ]
            : presets.value
        await writeTextFile(path, serializeGradientPresets(exportValue as GradientPreset[]))
    } catch (error) {
        handleError(error)
    }
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

function formatAdapter(adapterId: GradientFormatId): string {
    const adapter = gradientFormatAdapters.find(
        (item) => item.id === adapterId,
    ) as GradientFormatAdapter
    return `${formatAdapterName(adapterId)} · ${adapter.sample}`
}

function formatAdapterName(adapterId: GradientFormatId): string {
    return formatMessage(adapterMessages[adapterId])
}
</script>

<template>
    <main class="mx-auto flex w-full max-w-[90rem] flex-col gap-5 p-6">
        <header class="flex min-w-0 flex-wrap items-center justify-between gap-4">
            <h1 class="m-0 min-w-0 truncate text-2xl font-bold text-[var(--color-text-primary)]">
                {{ formatMessage(messages.title) }}
            </h1>
            <div
                class="flex min-w-0 flex-1 flex-wrap items-center justify-end gap-2 max-[680px]:w-full max-[680px]:justify-start"
            >
                <div class="lab-format-control">
                    <span>{{ formatMessage(messages.formatTitle) }}</span>
                    <Combobox
                        v-model="adapterId"
                        :options="
                            gradientFormatAdapters.map((adapter) => ({
                                value: adapter.id,
                                label: formatAdapter(adapter.id),
                            }))
                        "
                        class="max-w-[21rem] max-[680px]:max-w-none"
                    />
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
                                :highlighted="activeTextFormats.includes(format)"
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
                        class="lab-editor min-h-44 max-h-80 overflow-y-auto rounded-lg bg-surface-4 px-3 py-2.5 text-base leading-6 text-[var(--color-text-primary)] outline-none transition-shadow focus:ring-4 focus:ring-brand-shadow"
                        @input="onEditorInput"
                        @focus="syncTextFormatState"
                        @keyup="syncTextFormatState"
                        @mouseup="syncTextFormatState"
                        @paste="onEditorPaste"
                    ></div>
                </section>

                <section class="lab-panel-section p-4 lab-colors-section">
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
                                :aria-label="formatMessage(messages.randomizeColors)"
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
                                    :aria-label="formatMessage(messages.moveColorUp)"
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
                                    :aria-label="formatMessage(messages.moveColorDown)"
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
                                    :aria-label="formatMessage(messages.removeColor)"
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
                        <Button type="outlined" size="md" @click="applyImportedColors"
                            >{{ formatMessage(messages.importColors) }}
                        </Button>
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
                                    :aria-label="formatMessage(messages.deletePreset)"
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
                    class="lab-panel-section p-4 flex flex-wrap items-center gap-x-5 gap-y-3"
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

                <section class="lab-panel-section p-4 lab-preview-section">
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
                        :style="{ backgroundImage: `url(${minecraftPreviewBackground})` }"
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
    </main>
</template>

<style scoped>
@font-face {
    font-family: 'axolotl-lab-minecraft';
    font-style: normal;
    font-weight: 400;
    src: url(v-bind(minecraftPreviewFont)) format('truetype');
}

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

.lab-format-control :deep([data-combobox]) {
    width: min(21rem, 50vw);
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

.lab-color-swatch input,
.lab-new-color-picker {
    appearance: auto !important;
    -webkit-appearance: auto !important;
}

.lab-color-swatch input {
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
    font-family: 'axolotl-lab-minecraft', monospace;
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

@media (max-width: 65rem) {
    .lab-workbench {
        grid-template-columns: minmax(0, 1fr);
    }
}

@media (max-width: 42.5rem) {
    .lab-format-control {
        width: 100%;
    }

    .lab-format-control :deep([data-combobox]) {
        width: 100%;
    }
}

@media (max-width: 32rem) {
    .lab-color-row {
        grid-template-columns: 2.5rem minmax(0, 1fr);
    }

    .lab-color-row > :last-child {
        grid-column: 1 / -1;
        justify-content: flex-end;
    }
}
</style>
