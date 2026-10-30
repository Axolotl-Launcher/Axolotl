<script setup lang="ts">
import {
    BookIcon,
    BookmarkIcon,
    CheckIcon,
    ClipboardCopyIcon,
    CodeIcon,
    TrashIcon,
    XIcon,
} from '@modrinth/assets'
import {
    Button,
    Combobox,
    type ComboboxOption,
    defineMessages,
    StyledInput,
    Toggle,
    useVIntl,
} from '@modrinth/ui'
import { openUrl } from '@tauri-apps/plugin-opener'
import { computed, nextTick, onMounted, ref, watch } from 'vue'

import CommandGeneratorArgument from '@/components/lab/command-generator/CommandGeneratorArgument.vue'
import CommandGeneratorExecuteChain from '@/components/lab/command-generator/CommandGeneratorExecuteChain.vue'
import { findGuide } from '@/lab/command-generator/command-guides.ts'
import {
    CATEGORY_KEYS,
    findCommand,
    getAllCommands,
    getCurrentVersion,
    initializeRegistry,
    PERMISSION_DESC_KEYS,
    PERMISSION_DESCRIPTIONS,
    PERMISSION_KEYS,
    PERMISSION_LABELS,
    switchVersion,
} from '@/lab/command-generator/commands/registry.ts'
import type { CommandArgument, CommandCategory } from '@/lab/command-generator/commands/types.ts'
import {
    buildCommand,
    getActiveArguments,
    getDefaultValues,
    validateCommand,
} from '@/lab/command-generator/generator.ts'
import { loadManifest } from '@/lab/command-generator/loader.ts'
import {
    applyPresetToValues,
    deleteUserPreset,
    getUserPresets,
    saveUserPreset,
    type UserPreset,
} from '@/lab/command-generator/presets.ts'

const { formatMessage, locale } = useVIntl()

const messages = defineMessages({
    title: { id: 'app.lab.command-generator.title', defaultMessage: 'Command generator' },
    subtitle: {
        id: 'app.lab.command-generator.subtitle',
        defaultMessage: 'Build Minecraft Java Edition commands visually.',
    },
    globalPresets: {
        id: 'app.lab.command-generator.preset.global',
        defaultMessage: 'All presets',
    },
    globalPresetsEmpty: {
        id: 'app.lab.command-generator.preset.global-empty',
        defaultMessage: 'No saved presets yet.',
    },
    deletePreset: {
        id: 'app.lab.command-generator.preset.delete',
        defaultMessage: 'Delete preset',
    },
    close: { id: 'app.lab.command-generator.close', defaultMessage: 'Close' },
    selectVersion: {
        id: 'app.lab.command-generator.select-version',
        defaultMessage: 'Game version',
    },
    filters: { id: 'app.lab.command-generator.filters', defaultMessage: 'Filters' },
    filterCategory: {
        id: 'app.lab.command-generator.filter.category',
        defaultMessage: 'Category',
    },
    filterPermission: {
        id: 'app.lab.command-generator.filter.permission',
        defaultMessage: 'Permission level',
    },
    filterAll: { id: 'app.lab.command-generator.filter.all', defaultMessage: 'All' },
    selectCommand: {
        id: 'app.lab.command-generator.select-command',
        defaultMessage: 'Choose a command',
    },
    selectPlaceholder: {
        id: 'app.lab.command-generator.select-placeholder',
        defaultMessage: 'Search a command…',
    },
    aliasOf: {
        id: 'app.lab.command-generator.alias-of',
        defaultMessage: "{command}'s alias",
    },
    useDataComponents: {
        id: 'app.lab.command-generator.version.use-data-components',
        defaultMessage: 'Use data components (1.20.5+)',
    },
    startTitle: {
        id: 'app.lab.command-generator.start-title',
        defaultMessage: 'Choose a command to get started',
    },
    startHint: {
        id: 'app.lab.command-generator.start-hint',
        defaultMessage: 'Search by command name, or start with a common command.',
    },
    optional: { id: 'app.lab.command-generator.arg.optional', defaultMessage: 'Optional' },
    parametersEmpty: {
        id: 'app.lab.command-generator.parameters-empty',
        defaultMessage: 'This command takes no arguments.',
    },
    result: { id: 'app.lab.command-generator.result', defaultMessage: 'Generated command' },
    resultHint: {
        id: 'app.lab.command-generator.result-hint',
        defaultMessage: 'Fill in the parameters, then generate a command to copy into Minecraft.',
    },
    generate: { id: 'app.lab.command-generator.generate', defaultMessage: 'Generate' },
    copy: { id: 'app.lab.command-generator.copy', defaultMessage: 'Copy' },
    copied: { id: 'app.lab.command-generator.copied', defaultMessage: 'Copied' },
    copyFailed: {
        id: 'app.lab.command-generator.copy-failed',
        defaultMessage: 'Could not copy the command. Select and copy the text manually.',
    },
    unknownError: {
        id: 'app.lab.command-generator.error.unknown',
        defaultMessage: 'Unknown error',
    },
    requiredError: {
        id: 'app.lab.command-generator.error.required',
        defaultMessage: 'Enter {name}.',
    },
    enumError: {
        id: 'app.lab.command-generator.error.enum',
        defaultMessage: 'Choose a valid value for {name}.',
    },
    numberError: {
        id: 'app.lab.command-generator.error.number',
        defaultMessage: 'Enter a number for {name}.',
    },
    integerError: {
        id: 'app.lab.command-generator.error.integer',
        defaultMessage: 'Enter a whole number for {name}.',
    },
    rangeError: {
        id: 'app.lab.command-generator.error.range',
        defaultMessage: 'Enter a value within the allowed range for {name}.',
    },
    savePreset: {
        id: 'app.lab.command-generator.preset.save',
        defaultMessage: 'Save as preset',
    },
    presetName: {
        id: 'app.lab.command-generator.preset.name',
        defaultMessage: 'Preset name',
    },
    save: { id: 'app.lab.command-generator.save', defaultMessage: 'Save' },
    toastPresetNameRequired: {
        id: 'app.lab.command-generator.preset.toast.name-required',
        defaultMessage: 'Please enter a preset name',
    },
    toastPresetSaved: {
        id: 'app.lab.command-generator.preset.toast.saved',
        defaultMessage: 'Preset "{name}" saved',
    },
    toastPresetLoaded: {
        id: 'app.lab.command-generator.preset.toast.loaded',
        defaultMessage: 'Preset "{name}" loaded',
    },
    toastPresetDeleted: {
        id: 'app.lab.command-generator.preset.toast.deleted',
        defaultMessage: 'Preset "{name}" deleted',
    },
    toastPresetCommandMissing: {
        id: 'app.lab.command-generator.preset.toast.command-missing',
        defaultMessage:
            'Preset "{name}" uses command "{command}" which does not exist in this version',
    },
    executeBuildFirst: {
        id: 'app.lab.command-generator.execute.build-first',
        defaultMessage: 'Please build the subcommand chain first',
    },
    seedMultiplayerNote: {
        id: 'app.lab.command-generator.perm.seed.multiplayer-note',
        defaultMessage: 'In multiplayer, /seed requires permission level 2.',
    },
    guideTitle: {
        id: 'app.lab.command-generator.guide-title',
        defaultMessage: 'Guide',
    },
    guideExamples: {
        id: 'app.lab.command-generator.guide-examples',
        defaultMessage: 'Examples',
    },
    viewOnWiki: {
        id: 'app.lab.command-generator.view-on-wiki',
        defaultMessage: 'View on Wiki',
    },
})

function formatDynamic(id: string, fallback = ''): string {
    return formatMessage({ id, defaultMessage: fallback || id })
}

type CommandComboboxOption = ComboboxOption<string> & {
    description: string
    category: string
    isAlias?: boolean
    aliasOf?: string
}

function asCommandOption(item: ComboboxOption<string>): CommandComboboxOption {
    return item as CommandComboboxOption
}

const versionOptions = ref<ComboboxOption<string>[]>([])
const selectedVersion = ref('')
const selectedCommandId = ref('')
const filterCategory = ref<string>('')
const filterPermission = ref<string>('4')
const values = ref<Record<string, string | number | boolean>>({})
const executeChainOutput = ref('')
const executeChainRef = ref<InstanceType<typeof CommandGeneratorExecuteChain> | null>(null)
const result = ref('')
const errors = ref<string[]>([])
const copied = ref(false)
const presetNameInput = ref('')
const showPresetInput = ref(false)
const toast = ref('')
const showGlobalPresets = ref(false)

const userPresets = ref<UserPreset[]>([])
const useDataComponents = ref(true)

function isAtLeast(a: string, b: string): boolean {
    const av = a.split('.').map((n) => Number.parseInt(n, 10) || 0)
    const bv = b.split('.').map((n) => Number.parseInt(n, 10) || 0)
    const len = Math.max(av.length, bv.length)
    for (let i = 0; i < len; i++) {
        const x = av[i] ?? 0
        const y = bv[i] ?? 0
        if (x !== y) return x > y
    }
    return true
}

const showDataComponentsToggle = computed(() => {
    const v = selectedVersion.value
    if (!v) return false
    if (!(v === '1.20' || v.startsWith('1.20.'))) return false
    return !isAtLeast(v, '1.20.5')
})

const forceDataComponents = computed<boolean | undefined>(() => {
    if (!showDataComponentsToggle.value) return undefined
    return useDataComponents.value
})

const categoryOptions = computed<ComboboxOption<string>[]>(() => [
    { value: '', label: formatMessage(messages.filterAll) },
    ...(Object.keys(CATEGORY_KEYS) as CommandCategory[]).map((key) => ({
        value: key,
        label: formatDynamic(CATEGORY_KEYS[key], key),
    })),
])

const permissionOptions = computed<ComboboxOption<string>[]>(() =>
    ([0, 1, 2, 3, 4] as const).map((level) => ({
        value: String(level),
        label: `${level} · ${formatDynamic(PERMISSION_KEYS[level], PERMISSION_LABELS[level])} — ${formatDynamic(
            PERMISSION_DESC_KEYS[level],
            PERMISSION_DESCRIPTIONS[level],
        )}`,
    })),
)

const versionLabelMap = computed<Record<string, string>>(() => {
    const map: Record<string, string> = {}
    for (const opt of versionOptions.value) {
        map[String(opt.value)] = String(opt.label)
    }
    return map
})

const filteredCommands = computed(() => {
    let list = getAllCommands()
    if (filterCategory.value) {
        list = list.filter((c) => c.category === filterCategory.value)
    }
    const maxLevel = Number(filterPermission.value)
    if (!Number.isNaN(maxLevel)) {
        list = list.filter((c) => (c.permissionLevel ?? 0) <= maxLevel)
    }
    return list
})

const commandOptions = computed<CommandComboboxOption[]>(() => {
    const list: CommandComboboxOption[] = []
    for (const cmd of filteredCommands.value) {
        const description = formatDynamic(cmd.descriptionKey, cmd.name)
        const category = formatDynamic(CATEGORY_KEYS[cmd.category], cmd.category)

        list.push({
            value: cmd.id,
            label: cmd.name,
            description,
            category,
            searchTerms: [description, category],
        })

        for (const alias of cmd.aliases ?? []) {
            list.push({
                value: `alias:${cmd.id}:${alias}`,
                label: alias,
                description,
                category,
                isAlias: true,
                aliasOf: cmd.name,
                searchTerms: [cmd.name, description, category],
            })
        }
    }
    return list
})

const currentCommand = computed(() =>
    selectedCommandId.value ? (findCommand(selectedCommandId.value) ?? null) : null,
)

const currentGuide = computed(() =>
    selectedCommandId.value
        ? (findGuide(selectedCommandId.value, selectedVersion.value) ?? null)
        : null,
)

const wikiUrl = computed(() => {
    if (!currentCommand.value) return '#'
    const isZh = locale.value.startsWith('zh')
    const base = isZh ? 'https://zh.minecraft.wiki/w/命令/' : 'https://minecraft.wiki/w/Commands/'
    return `${base}${currentCommand.value.id}`
})

const uiArguments = computed<CommandArgument[]>(() => {
    const args = currentCommand.value ? getActiveArguments(currentCommand.value, values.value) : []
    const nonNbt: CommandArgument[] = []
    const nbt: CommandArgument[] = []
    for (const arg of args) {
        if (arg.literal !== undefined) continue
        if (arg.type === 'nbt') nbt.push(arg)
        else nonNbt.push(arg)
    }
    return [...nonNbt, ...nbt]
})

const permissionName = computed(() => {
    if (!currentCommand.value) return ''
    const level = currentCommand.value.permissionLevel
    return `${formatDynamic(PERMISSION_KEYS[level], PERMISSION_LABELS[level] ?? String(level))} (${level})`
})

const permissionTooltip = computed(() => {
    if (!currentCommand.value) return undefined
    if (currentCommand.value.id === 'seed') {
        return formatMessage(messages.seedMultiplayerNote)
    }
    return formatDynamic(
        PERMISSION_DESC_KEYS[currentCommand.value.permissionLevel],
        PERMISSION_DESCRIPTIONS[currentCommand.value.permissionLevel],
    )
})

const isChainCommand = computed(() => currentCommand.value?.isChainCommand === true)

const allPresets = computed<UserPreset[]>(() =>
    [...userPresets.value].sort((a, b) => b.createdAt - a.createdAt),
)

onMounted(() => {
    const manifest = loadManifest()
    versionOptions.value = manifest.versions.map((v) => ({
        value: v.id,
        label: v.label,
    }))
    selectedVersion.value = manifest.defaultVersion
    initializeRegistry(manifest.defaultVersion)
    userPresets.value = getUserPresets()
})

watch(selectedVersion, (versionId) => {
    if (!versionId) return
    errors.value = []
    switchVersion(versionId)
    selectedCommandId.value = ''
    values.value = {}
    executeChainOutput.value = ''
    result.value = ''
    useDataComponents.value = true
})

watch(selectedCommandId, () => {
    userPresets.value = getUserPresets()
})

function showToast(message: string) {
    toast.value = message
    setTimeout(() => {
        toast.value = ''
    }, 1800)
}

watch(
    [values, executeChainOutput],
    () => {
        result.value = ''
        copied.value = false
    },
    { deep: true },
)

function onCommandChange(id: unknown) {
    const rawId = String(id)
    let resolvedId = rawId
    if (rawId.startsWith('alias:')) {
        const parts = rawId.split(':')
        resolvedId = parts[1] ?? ''
    }
    selectedCommandId.value = resolvedId
    copied.value = false
    executeChainOutput.value = ''
    if (!selectedCommandId.value) {
        values.value = {}
        result.value = ''
        errors.value = []
        return
    }
    values.value = getDefaultValues(selectedCommandId.value)
    result.value = ''
    errors.value = []
}

function generate() {
    if (!selectedCommandId.value || !currentCommand.value) return
    copied.value = false

    if (isChainCommand.value) {
        const output = executeChainOutput.value.trim()
        if (!output) {
            errors.value = [formatMessage(messages.executeBuildFirst)]
            result.value = ''
            return
        }
        errors.value = []
        result.value = `/${selectedCommandId.value} ${output}`
        return
    }

    const validationErrors = validateCommand(selectedCommandId.value, values.value, (issue) =>
        formatMessage(messages[issue.kind], {
            name: formatDynamic(issue.argument.displayNameKey, issue.argument.name),
        }),
    )
    if (validationErrors.length) {
        errors.value = validationErrors
        result.value = ''
        return
    }
    errors.value = []
    try {
        result.value = buildCommand(selectedCommandId.value, values.value)
    } catch (e) {
        errors.value = [e instanceof Error ? e.message : formatMessage(messages.unknownError)]
        result.value = ''
    }
}

async function copy() {
    if (!result.value) return
    try {
        await navigator.clipboard.writeText(result.value)
        copied.value = true
        setTimeout(() => {
            copied.value = false
        }, 1600)
    } catch {
        showToast(formatMessage(messages.copyFailed))
    }
}

function saveCurrentAsPreset() {
    if (!selectedCommandId.value || !currentCommand.value) return
    const name = presetNameInput.value.trim()
    if (!name) {
        showToast(formatMessage(messages.toastPresetNameRequired))
        return
    }

    if (isChainCommand.value) {
        const state = executeChainRef.value?.getState()
        if (!state) {
            showToast(formatMessage(messages.executeBuildFirst))
            return
        }
        saveUserPreset(
            name,
            selectedCommandId.value,
            getCurrentVersion(),
            {},
            {
                executeState: state,
                chainString: executeChainOutput.value,
            },
        )
    } else {
        const snapshot: Record<string, string | number | boolean> = {}
        for (const arg of currentCommand.value.arguments) {
            snapshot[arg.name] = values.value[arg.name] ?? ''
        }
        saveUserPreset(name, selectedCommandId.value, getCurrentVersion(), snapshot)
    }

    userPresets.value = getUserPresets()
    presetNameInput.value = ''
    showPresetInput.value = false
    showToast(formatMessage(messages.toastPresetSaved, { name }))
}

function removePreset(id: string) {
    const preset = userPresets.value.find((p) => p.id === id)
    if (!preset) return

    deleteUserPreset(id)
    userPresets.value = getUserPresets()
    showToast(formatMessage(messages.toastPresetDeleted, { name: preset.name }))
}

async function jumpToPreset(preset: UserPreset) {
    if (selectedVersion.value !== preset.versionId) {
        selectedVersion.value = preset.versionId
        await nextTick()
    }

    filterCategory.value = ''
    filterPermission.value = '4'

    selectedCommandId.value = preset.commandId
    const cmd = findCommand(preset.commandId)
    if (!cmd) {
        showToast(
            formatMessage(messages.toastPresetCommandMissing, {
                name: preset.name,
                command: preset.commandId,
            }),
        )
        showGlobalPresets.value = false
        return
    }

    await nextTick()
    if (cmd.isChainCommand) {
        if (preset.executeState) {
            executeChainRef.value?.setState(preset.executeState)
        } else if (preset.chainString) {
            executeChainOutput.value = preset.chainString
        }
    } else {
        values.value = applyPresetToValues(preset, cmd)
    }
    result.value = ''
    errors.value = []

    showGlobalPresets.value = false
    showToast(formatMessage(messages.toastPresetLoaded, { name: preset.name }))
}

async function openWiki() {
    if (!currentCommand.value) return
    try {
        await openUrl(wikiUrl.value)
    } catch (e) {
        console.error('Failed to open wiki link:', e)
    }
}
</script>

<template>
    <main
        class="command-generator mx-auto flex w-full min-w-0 max-w-[90rem] flex-col gap-5 p-4 sm:p-6"
    >
        <header class="flex min-w-0 flex-wrap items-center justify-between gap-3">
            <div class="min-w-0">
                <h1 class="m-0 text-2xl font-bold text-[var(--color-text-primary)]">
                    {{ formatMessage(messages.title) }}
                </h1>
                <p class="m-0 mt-1 text-sm text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.subtitle) }}
                </p>
            </div>
            <Button
                class="global-presets-button"
                type="outlined"
                :aria-expanded="showGlobalPresets"
                @click="showGlobalPresets = !showGlobalPresets"
            >
                <BookmarkIcon />{{ formatMessage(messages.globalPresets) }}
                <span class="rounded-md bg-surface-3 px-1.5 text-xs tabular-nums">{{
                    allPresets.length
                }}</span>
            </Button>
        </header>

        <section
            v-if="showGlobalPresets"
            class="rounded-xl border border-solid border-surface-4 bg-surface-2 p-4"
        >
            <div class="mb-3 flex items-center justify-between gap-3">
                <h2 class="m-0 text-base font-semibold text-[var(--color-text-primary)]">
                    {{ formatMessage(messages.globalPresets) }}
                </h2>
                <Button
                    v-tooltip="formatMessage(messages.close)"
                    type="quiet"
                    icon-only
                    :label="formatMessage(messages.close)"
                    @click="showGlobalPresets = false"
                    ><XIcon
                /></Button>
            </div>
            <p v-if="!allPresets.length" class="m-0 text-sm text-[var(--color-text-tertiary)]">
                {{ formatMessage(messages.globalPresetsEmpty) }}
            </p>
            <div v-else class="grid min-w-0 gap-2 sm:grid-cols-2 lg:grid-cols-3">
                <div
                    v-for="preset in allPresets"
                    :key="preset.id"
                    class="flex min-w-0 items-center gap-1 rounded-lg border border-solid border-surface-4 bg-surface-3 p-2"
                >
                    <button
                        class="global-preset-item flex min-w-0 flex-1 flex-col gap-1 rounded-md bg-transparent p-1 text-left hover:text-brand focus-visible:outline-brand"
                        @click="jumpToPreset(preset)"
                    >
                        <span class="w-full truncate text-sm font-semibold">{{ preset.name }}</span>
                        <span class="text-xs text-[var(--color-text-tertiary)]"
                            >/{{ preset.commandId }} ·
                            {{ versionLabelMap[preset.versionId] ?? preset.versionId }}</span
                        >
                    </button>
                    <Button
                        v-tooltip="formatMessage(messages.deletePreset)"
                        type="quiet"
                        icon-only
                        size="sm"
                        :label="formatMessage(messages.deletePreset)"
                        @click="removePreset(preset.id)"
                        ><TrashIcon
                    /></Button>
                </div>
            </div>
        </section>

        <div
            class="grid min-w-0 items-start gap-5 lg:grid-cols-[minmax(0,1fr)_minmax(18rem,24rem)]"
        >
            <section class="min-w-0 rounded-xl border border-solid border-surface-4 bg-surface-2">
                <div
                    class="flex min-w-0 flex-col gap-4 border-0 border-b border-solid border-surface-4 p-4"
                >
                    <div class="flex flex-col items-start gap-3">
                        <div class="flex min-w-0 flex-col gap-2">
                            <label class="text-sm font-semibold text-[var(--color-text-primary)]">{{
                                formatMessage(messages.selectVersion)
                            }}</label>
                            <Combobox
                                :model-value="selectedVersion"
                                :options="versionOptions"
                                @update:model-value="selectedVersion = String($event)"
                            />
                        </div>
                        <div class="flex min-w-0 flex-col gap-2">
                            <span class="text-sm font-semibold text-[var(--color-text-primary)]">
                                {{ formatMessage(messages.filters) }}
                            </span>
                            <div class="flex flex-wrap gap-3">
                                <div class="flex min-w-0 flex-col gap-2">
                                    <label class="text-sm">{{
                                        formatMessage(messages.filterCategory)
                                    }}</label
                                    ><Combobox
                                        :model-value="filterCategory"
                                        :options="categoryOptions"
                                        @update:model-value="filterCategory = String($event)"
                                    />
                                </div>
                                <div class="flex min-w-0 flex-col gap-2">
                                    <label class="text-sm">{{
                                        formatMessage(messages.filterPermission)
                                    }}</label
                                    ><Combobox
                                        :model-value="filterPermission"
                                        :options="permissionOptions"
                                        width="full"
                                        @update:model-value="filterPermission = String($event)"
                                    />
                                </div>
                            </div>
                        </div>
                    </div>
                    <div class="flex min-w-0 flex-col gap-2">
                        <label class="text-sm font-semibold text-[var(--color-text-primary)]">{{
                            formatMessage(messages.selectCommand)
                        }}</label>
                        <Combobox
                            :model-value="selectedCommandId"
                            :options="commandOptions"
                            :placeholder="formatMessage(messages.selectPlaceholder)"
                            width="full"
                            searchable
                            @update:model-value="onCommandChange"
                        >
                            <template #option="{ item }">
                                <div class="flex min-w-0 flex-col gap-1">
                                    <span class="flex flex-wrap items-baseline gap-1.5">
                                        <span class="font-mono font-semibold"
                                            >/{{ item.label }}</span
                                        >
                                        <span
                                            v-if="asCommandOption(item).isAlias"
                                            class="inline-flex items-center rounded bg-surface-3 px-1.5 py-0.5 text-[0.65rem] leading-none text-[var(--color-text-tertiary)]"
                                        >
                                            {{
                                                formatMessage(messages.aliasOf, {
                                                    command: asCommandOption(item).aliasOf,
                                                })
                                            }}
                                        </span>
                                    </span>
                                    <span class="text-xs text-[var(--color-text-tertiary)]">{{
                                        asCommandOption(item).description
                                    }}</span>
                                </div>
                            </template>
                        </Combobox>
                    </div>
                    <div
                        v-if="showDataComponentsToggle"
                        class="flex items-center justify-between gap-3 rounded-lg bg-surface-3 p-3 text-sm"
                    >
                        <span>{{ formatMessage(messages.useDataComponents) }}</span>
                        <Toggle
                            v-model="useDataComponents"
                            :aria-label="formatMessage(messages.useDataComponents)"
                        />
                    </div>
                </div>

                <div v-if="!currentCommand" class="flex flex-col gap-4 p-5">
                    <div>
                        <h2 class="m-0 text-base font-semibold text-[var(--color-text-primary)]">
                            {{ formatMessage(messages.startTitle) }}
                        </h2>
                        <p class="m-0 mt-2 text-sm leading-6 text-[var(--color-text-tertiary)]">
                            {{ formatMessage(messages.startHint) }}
                        </p>
                    </div>
                    <div class="flex flex-wrap gap-2">
                        <Button
                            v-for="id in ['give', 'execute', 'summon', 'gamemode']"
                            :key="id"
                            type="outlined"
                            @click="onCommandChange(id)"
                            ><code>/{{ id }}</code></Button
                        >
                    </div>
                </div>

                <div v-else class="parameters-section flex min-w-0 flex-col gap-5 p-4">
                    <div class="flex flex-wrap items-start justify-between gap-3">
                        <div class="min-w-0">
                            <h2
                                class="m-0 font-mono text-lg font-semibold text-[var(--color-text-primary)]"
                            >
                                /{{ currentCommand.name }}
                            </h2>
                            <p class="m-0 mt-1 text-sm leading-6 text-[var(--color-text-tertiary)]">
                                {{
                                    formatDynamic(
                                        currentCommand.descriptionKey,
                                        currentCommand.name,
                                    )
                                }}
                            </p>
                        </div>
                        <span
                            v-tooltip="permissionTooltip"
                            class="rounded-md bg-surface-3 px-2 py-1 text-xs text-[var(--color-text-tertiary)]"
                            >{{ permissionName }}</span
                        >
                    </div>
                    <CommandGeneratorExecuteChain
                        v-if="isChainCommand && currentCommand.chainCategories"
                        ref="executeChainRef"
                        v-model="executeChainOutput"
                        :categories="currentCommand.chainCategories"
                    />
                    <div
                        v-else-if="uiArguments.length"
                        class="arguments-grid grid min-w-0 gap-x-5 gap-y-5 md:grid-cols-2"
                    >
                        <template v-for="arg in uiArguments" :key="arg.name">
                            <details
                                v-if="arg.type === 'nbt' && !arg.required"
                                class="min-w-0 rounded-lg border border-solid border-surface-4 p-3 md:col-span-2"
                            >
                                <summary
                                    class="cursor-pointer text-sm font-semibold text-[var(--color-text-primary)]"
                                >
                                    {{ formatDynamic(arg.displayNameKey, arg.name) }}
                                    <span
                                        class="ml-2 font-normal text-[var(--color-text-tertiary)]"
                                        >{{ formatMessage(messages.optional) }}</span
                                    >
                                </summary>
                                <CommandGeneratorArgument
                                    v-model="values[arg.name]"
                                    class="mt-4"
                                    :arg="arg"
                                    :version="selectedVersion"
                                    :force-data-components="forceDataComponents"
                                />
                            </details>
                            <CommandGeneratorArgument
                                v-else
                                v-model="values[arg.name]"
                                :arg="arg"
                                :version="selectedVersion"
                                :force-data-components="forceDataComponents"
                            />
                        </template>
                    </div>
                    <p v-else class="m-0 text-sm text-[var(--color-text-tertiary)]">
                        {{ formatMessage(messages.parametersEmpty) }}
                    </p>
                </div>
            </section>

            <aside class="flex min-w-0 flex-col gap-4 lg:sticky lg:top-4">
                <section
                    class="rounded-xl border border-solid border-surface-4 bg-surface-2 p-4"
                    aria-live="polite"
                >
                    <div class="mb-4 flex items-center gap-2">
                        <CodeIcon class="h-5 w-5 text-[var(--color-text-tertiary)]" />
                        <h2 class="m-0 text-base font-semibold text-[var(--color-text-primary)]">
                            {{ formatMessage(messages.result) }}
                        </h2>
                    </div>
                    <div
                        v-if="errors.length"
                        role="alert"
                        class="mb-4 rounded-lg bg-red-highlight p-3 text-sm text-red"
                    >
                        <ul class="m-0 pl-4">
                            <li v-for="(err, i) in errors" :key="i">{{ err }}</li>
                        </ul>
                    </div>
                    <pre
                        v-if="result"
                        class="result-code m-0 mb-4 max-h-72 overflow-auto whitespace-pre-wrap break-all rounded-lg bg-surface-3 p-3 font-mono text-sm leading-6 text-[var(--color-text-primary)]"
                        >{{ result }}</pre>
                    <p v-else class="m-0 mb-4 text-sm leading-6 text-[var(--color-text-tertiary)]">
                        {{
                            formatMessage(currentCommand ? messages.resultHint : messages.startHint)
                        }}
                    </p>
                    <div class="action-bar flex flex-wrap gap-2">
                        <Button
                            type="colored"
                            color="brand"
                            :disabled="!currentCommand"
                            @click="generate"
                            >{{ formatMessage(messages.generate) }}</Button
                        >
                        <Button type="outlined" :disabled="!result" @click="copy"
                            ><CheckIcon v-if="copied" /><ClipboardCopyIcon v-else />{{
                                formatMessage(copied ? messages.copied : messages.copy)
                            }}</Button
                        >
                    </div>

                    <div
                        v-if="currentCommand"
                        class="mt-4 flex flex-wrap items-center gap-2 border-0 border-t border-solid border-surface-4 pt-3"
                    >
                        <Button
                            type="quiet"
                            size="sm"
                            :aria-expanded="showPresetInput"
                            @click="showPresetInput = !showPresetInput"
                        >
                            <BookmarkIcon />{{ formatMessage(messages.savePreset) }}
                        </Button>

                        <Button type="quiet" size="sm" @click="openWiki">
                            <BookIcon class="h-4 w-4" />
                            {{ formatMessage(messages.viewOnWiki) }}
                        </Button>

                        <form
                            v-if="showPresetInput"
                            class="mt-3 flex w-full min-w-0 flex-wrap items-center gap-2"
                            @submit.prevent="saveCurrentAsPreset"
                        >
                            <StyledInput
                                v-model="presetNameInput"
                                class="min-w-0 flex-1"
                                :placeholder="formatMessage(messages.presetName)"
                                :aria-label="formatMessage(messages.presetName)"
                            />
                            <Button native-type="submit" type="outlined" size="sm">{{
                                formatMessage(messages.save)
                            }}</Button>
                        </form>
                    </div>

                    <p
                        v-if="toast"
                        role="status"
                        class="m-0 mt-3 text-sm text-[var(--color-text-default)]"
                    >
                        {{ toast }}
                    </p>
                </section>

                <section
                    v-if="currentGuide"
                    class="rounded-xl border border-solid border-surface-4 bg-surface-2 p-4"
                >
                    <h2 class="m-0 text-base font-semibold text-[var(--color-text-primary)]">
                        {{ formatMessage(messages.guideTitle) }}
                    </h2>
                    <p class="m-0 mt-2 text-sm leading-6 text-[var(--color-text-tertiary)]">
                        {{ formatDynamic(currentGuide.summaryKey, currentGuide.commandId) }}
                    </p>
                    <details class="mt-3">
                        <summary class="cursor-pointer text-sm font-medium">
                            {{ formatMessage(messages.guideExamples) }}
                        </summary>
                        <div class="mt-3 flex flex-col gap-3">
                            <div
                                v-for="ex in currentGuide.examples"
                                :key="ex.command"
                                class="min-w-0"
                            >
                                <code
                                    class="block break-all rounded-lg bg-surface-3 p-2 text-xs leading-5"
                                    >{{ ex.command }}</code
                                >
                                <p
                                    class="m-0 mt-1 text-xs leading-5 text-[var(--color-text-tertiary)]"
                                >
                                    {{ formatDynamic(ex.explanationKey, ex.command) }}
                                </p>
                            </div>
                            <p
                                v-for="tipKey in currentGuide.tipKeys ?? []"
                                :key="tipKey"
                                class="m-0 text-sm leading-6 text-[var(--color-text-tertiary)]"
                            >
                                {{ formatDynamic(tipKey) }}
                            </p>
                        </div>
                    </details>
                </section>
            </aside>
        </div>
    </main>
</template>
