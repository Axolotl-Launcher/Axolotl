<template>
    <NewModal ref="unsavedModal">
        <template #title>
            <span class="font-extrabold text-lg text-[var(--color-text-primary)]">
                {{ formatMessage(messages.unsavedTitle) }}
            </span>
        </template>
        <div class="w-[400px] max-w-full">
            <p class="m-0">{{ formatMessage(messages.unsavedBody) }}</p>
        </div>
        <div class="flex gap-2 mt-4">
            <Button type="colored" color="red" @click="confirmLeave"
                ><TrashIcon />
                {{ formatMessage(messages.leaveButton) }}
            </Button>
            <Button @click="unsavedModal?.hide()"
                ><XIcon />
                {{ formatMessage(messages.stayButton) }}
            </Button>
        </div>
    </NewModal>
    <EmptyState
        v-if="loadError"
        type="error"
        :heading="formatMessage(messages.loadErrorHeading)"
        :description="loadError"
    />
    <div v-else-if="data" class="flex flex-col gap-6 pb-4">
        <div class="flex items-center gap-4">
            <div class="group relative">
                <Avatar :src="form.removeIcon ? undefined : data.icon" size="64px" />
                <button
                    v-if="data.icon && !form.removeIcon && !readonly"
                    v-tooltip="formatMessage(messages.resetIcon)"
                    class="absolute inset-0 hidden cursor-pointer items-center justify-center rounded-xl border-none bg-black/60 text-white group-hover:flex"
                    @click="form.removeIcon = true"
                >
                    <UndoIcon class="size-5" />
                </button>
            </div>
            <div class="flex min-w-0 flex-col gap-1.5">
                <h1 class="m-0 truncate text-2xl font-extrabold text-[var(--color-text-primary)]">
                    {{ data.name }}
                </h1>
                <div
                    class="flex flex-wrap items-center gap-2 text-sm text-[var(--color-text-tertiary)]"
                >
                    <span
                        v-if="data.version_name"
                        class="rounded-full bg-surface-4 px-2 py-0.5 font-medium"
                    >
                        {{ data.version_name }}
                    </span>
                    <span
                        v-if="data.modded"
                        class="rounded-full bg-surface-4 px-2 py-0.5 font-medium"
                    >
                        {{ formatMessage(messages.moddedBadge) }}
                    </span>
                    <span
                        v-if="data.hardcore"
                        class="rounded-full bg-bg-red px-2 py-0.5 font-medium text-red"
                    >
                        {{ formatMessage(messages.hardcoreBadge) }}
                    </span>
                    <span v-if="data.last_played">
                        {{
                            formatMessage(messages.lastPlayed, {
                                ago: formatRelativeTime(dayjs(data.last_played).toISOString()),
                            })
                        }}
                    </span>
                </div>
            </div>
        </div>

        <Admonition v-if="readonly" type="warning" :header="formatMessage(messages.lockedHeading)">
            {{ formatMessage(messages.lockedBody) }}
        </Admonition>
        <SymlinkInstanceWarning
            v-if="instance?.symlink_target"
            :symlink-target="instance.symlink_target"
        />

        <section class="flex flex-col gap-3">
            <h2 class="m-0 text-lg font-extrabold text-[var(--color-text-primary)]">
                {{ formatMessage(messages.basicSection) }}
            </h2>
            <div class="grid grid-cols-1 gap-4 sm:grid-cols-2">
                <div class="flex flex-col gap-1.5">
                    <label class="font-semibold text-[var(--color-text-primary)]" for="world-name">
                        {{ formatMessage(messages.nameLabel) }}
                    </label>
                    <StyledInput
                        id="world-name"
                        v-model="form.name"
                        :placeholder="formatMessage(messages.namePlaceholder)"
                        autocomplete="off"
                        :disabled="readonly"
                        wrapper-class="w-full"
                    />
                    <span v-if="nameError" class="text-sm text-red">
                        {{ formatMessage(messages.nameRequired) }}
                    </span>
                </div>
                <div class="flex flex-col gap-1.5">
                    <span class="font-semibold text-[var(--color-text-primary)]">
                        {{ formatMessage(messages.gameModeLabel) }}
                    </span>
                    <Combobox
                        v-model="form.gameMode"
                        :options="
                            GAME_MODE_OPTIONS.map((value) => ({
                                value,
                                label: gameModeLabel(value),
                            }))
                        "
                        :disabled="readonly"
                    />
                </div>
                <div v-if="form.difficulty" class="flex flex-col gap-1.5">
                    <span class="font-semibold text-[var(--color-text-primary)]">
                        {{ formatMessage(messages.difficultyLabel) }}
                        <span
                            v-if="data.difficulty_locked"
                            class="font-normal text-[var(--color-text-tertiary)]"
                        >
                            {{ formatMessage(messages.difficultyLockedHint) }}
                        </span>
                    </span>
                    <Combobox
                        v-model="form.difficulty"
                        :options="
                            DIFFICULTY_OPTIONS.map((value) => ({
                                value,
                                label: difficultyLabel(value),
                            }))
                        "
                        :disabled="readonly"
                    />
                </div>
                <div v-if="form.allowCommands !== undefined" class="flex flex-col gap-1.5">
                    <span class="font-semibold text-[var(--color-text-primary)]">
                        {{ formatMessage(messages.allowCommandsLabel) }}
                    </span>
                    <Combobox
                        v-model="form.allowCommands"
                        :options="
                            BOOLEAN_OPTIONS.map((value) => ({ value, label: booleanLabel(value) }))
                        "
                        :disabled="readonly"
                    />
                </div>
            </div>
        </section>

        <section v-if="form.seed !== undefined" class="flex flex-col gap-3">
            <h2 class="m-0 text-lg font-extrabold text-[var(--color-text-primary)]">
                {{ formatMessage(messages.seedSection) }}
            </h2>
            <Admonition type="warning" :header="formatMessage(messages.seedWarningHeading)">
                {{ formatMessage(messages.seedWarningBody) }}
            </Admonition>
            <div class="flex max-w-md flex-col gap-1.5">
                <StyledInput
                    v-model="form.seed"
                    autocomplete="off"
                    :spellcheck="false"
                    input-class="font-mono"
                    :disabled="readonly"
                    wrapper-class="w-full"
                />
                <span v-if="seedError" class="text-sm text-red">
                    {{ formatMessage(messages.seedInvalid) }}
                </span>
            </div>
        </section>

        <section v-if="data.game_rules.length > 0" class="flex flex-col gap-3">
            <h2 class="m-0 text-lg font-extrabold text-[var(--color-text-primary)]">
                {{ formatMessage(messages.gameRulesSection) }}
            </h2>
            <StyledInput
                v-model="ruleSearch"
                :icon="SearchIcon"
                type="text"
                autocomplete="off"
                :spellcheck="false"
                clearable
                :placeholder="
                    formatMessage(messages.searchRulesPlaceholder, {
                        count: data.game_rules.length,
                    })
                "
                wrapper-class="max-w-md"
            />
            <div v-if="ruleGroups.length === 0" class="text-[var(--color-text-tertiary)]">
                {{ formatMessage(messages.noRulesFound) }}
            </div>
            <div v-for="group in ruleGroups" :key="group.category" class="flex flex-col gap-2">
                <Accordion
                    :open-by-default="false"
                    :force-open="ruleSearchActive"
                    class="min-w-0 overflow-hidden rounded-xl border border-solid border-surface-4 bg-surface-2"
                    button-class="group flex w-full cursor-pointer items-center gap-3 border-0 bg-transparent px-4 py-3 text-left"
                >
                    <template #title>
                        <h3 class="m-0 text-base font-bold text-[var(--color-text-primary)]">
                            {{ group.label }}
                        </h3>
                    </template>
                    <div class="border-0 border-t border-solid border-surface-4">
                        <div
                            v-for="rule in group.rules"
                            :key="rule.key"
                            class="flex flex-wrap items-center justify-between gap-2 border-b border-solid border-surface-4 px-4 py-2.5 last:border-b-0"
                        >
                            <div class="flex min-w-0 items-center gap-2">
                                <span
                                    v-if="rule.modifiedFromDefault"
                                    v-tooltip="formatMessage(messages.modifiedFromDefault)"
                                    class="size-2 shrink-0 rounded-full bg-brand"
                                />
                                <span
                                    class="truncate font-medium text-[var(--color-text-primary)]"
                                    :title="rule.key"
                                >
                                    {{ rule.label }}
                                </span>
                            </div>
                            <div class="flex items-center gap-1.5">
                                <Button
                                    v-if="rule.canResetToDefault"
                                    v-tooltip="formatMessage(messages.resetRuleToDefault)"
                                    type="quiet"
                                    size="2xs"
                                    :disabled="readonly"
                                    @click="resetRuleToDefault(rule.key, rule.defaultValue)"
                                >
                                    <UndoIcon />
                                </Button>
                                <Combobox
                                    v-if="rule.widget === 'boolean'"
                                    v-model="form.rules[rule.key]"
                                    width="custom"
                                    class="w-36"
                                    :options="
                                        BOOLEAN_OPTIONS.map((value) => ({
                                            value,
                                            label: booleanLabel(value),
                                        }))
                                    "
                                    :disabled="readonly"
                                    force-direction="up"
                                />
                                <StyledInput
                                    v-else
                                    v-model="form.rules[rule.key]"
                                    autocomplete="off"
                                    :spellcheck="false"
                                    :disabled="readonly"
                                    input-class="font-mono !h-9"
                                    wrapper-class="w-36"
                                />
                            </div>
                            <span
                                v-if="invalidRules.includes(rule.key)"
                                class="w-full text-sm text-red"
                            >
                                {{ formatMessage(messages.ruleInvalidInteger) }}
                            </span>
                        </div>
                    </div>
                </Accordion>
            </div>
        </section>

        <div
            v-if="dirty && !readonly"
            class="sticky bottom-0 z-10 -mx-2 flex flex-wrap items-center gap-3 rounded-t-xl border border-b-0 border-solid border-surface-4 bg-surface-3 px-4 py-3 shadow-lg"
        >
            <span class="font-semibold text-[var(--color-text-primary)]">
                {{ formatMessage(messages.unsavedChangesLabel) }}
            </span>
            <div class="ml-auto flex gap-2">
                <Button type="colored" color="brand" :disabled="!canSave" @click="save"
                    ><SaveIcon />
                    {{ formatMessage(commonMessages.saveChangesButton) }}
                </Button>
                <Button :disabled="saving" @click="discard"
                    ><XIcon />
                    {{ formatMessage(messages.discardButton) }}
                </Button>
            </div>
        </div>
    </div>
</template>

<script setup lang="ts">
import { SaveIcon, SearchIcon, TrashIcon, UndoIcon, XIcon } from '@modrinth/assets'
import {
    Accordion,
    Admonition,
    Avatar,
    Button,
    Combobox,
    commonMessages,
    defineMessages,
    EmptyState,
    GAME_MODES,
    injectNotificationManager,
    NewModal,
    StyledInput,
    useRelativeTime,
    useVIntl,
} from '@modrinth/ui'
import { useQueryClient } from '@tanstack/vue-query'
import dayjs from 'dayjs'
import { computed, ref, watch } from 'vue'
import { onBeforeRouteLeave, type RouteLocationNormalized, useRoute, useRouter } from 'vue-router'

import SymlinkInstanceWarning from '@/components/ui/SymlinkInstanceWarning.vue'
import {
    gameRuleCategoryMessages,
    getGameRuleMetadata,
    resolveGameRuleType,
} from '@/components/ui/world/gameRuleRegistry.ts'
import type { GameInstance } from '@/helpers/types'
import {
    get_world_level_data,
    reset_world_icon,
    type SingleplayerGameMode,
    update_world_settings,
    type WorldDifficulty,
    type WorldLevelData,
    type WorldSettingsPatch,
} from '@/helpers/worlds.ts'

const { formatMessage } = useVIntl()
const { addNotification, handleError } = injectNotificationManager()
const formatRelativeTime = useRelativeTime()
const route = useRoute()
const router = useRouter()
const queryClient = useQueryClient()

const props = defineProps<{
    instance: GameInstance
    options?: unknown
    offline?: boolean
    playing?: boolean
    installed?: boolean
}>()

const worldPath = computed(() => decodeURIComponent(String(route.params.world ?? '')))

const GAME_MODE_OPTIONS: SingleplayerGameMode[] = ['survival', 'creative', 'adventure', 'spectator']
const DIFFICULTY_OPTIONS: WorldDifficulty[] = ['peaceful', 'easy', 'normal', 'hard']
const BOOLEAN_OPTIONS = ['true', 'false']

const RULE_CATEGORY_ORDER = [
    'player',
    'mobs',
    'drops',
    'world',
    'chat',
    'commands',
    'other',
] as const

type FormState = {
    name: string
    gameMode: SingleplayerGameMode
    difficulty?: WorldDifficulty
    allowCommands?: string
    seed?: string
    rules: Record<string, string>
    removeIcon: boolean
}

const data = ref<WorldLevelData>()
const form = ref<FormState>(emptyForm())
const savedState = ref<FormState>(emptyForm())
const loadError = ref<string>()
const saving = ref(false)
const ruleSearch = ref('')

const ruleSearchActive = computed(() => ruleSearch.value.trim().length > 0)

function emptyForm(): FormState {
    return { name: '', gameMode: 'survival', rules: {}, removeIcon: false }
}

function snapshotForm(level: WorldLevelData): FormState {
    return {
        name: level.name,
        gameMode: level.game_mode,
        difficulty: level.difficulty,
        allowCommands:
            level.allow_commands === undefined ? undefined : String(level.allow_commands),
        seed: level.seed,
        rules: Object.fromEntries(level.game_rules.map((rule) => [rule.key, rule.value])),
        removeIcon: false,
    }
}

async function load() {
    try {
        const level = await get_world_level_data(props.instance.id, worldPath.value)
        data.value = level
        form.value = snapshotForm(level)
        savedState.value = snapshotForm(level)
        loadError.value = undefined
    } catch (err) {
        loadError.value = err instanceof Error ? err.message : String(err)
    }
}

await load()

watch(
    () => props.playing,
    (playing) => {
        if (!playing) {
            setTimeout(() => {
                if (!dirty.value) {
                    load()
                } else if (data.value) {
                    reloadLockedStateOnly()
                }
            }, 1000)
        }
    },
)

async function reloadLockedStateOnly() {
    try {
        const level = await get_world_level_data(props.instance.id, worldPath.value)
        if (data.value) {
            data.value.locked = level.locked
        }
    } catch {
        // Keep the current editor state when only the lock probe fails
    }
}

const readonly = computed(() => data.value?.locked ?? true)

const dirty = computed(() => JSON.stringify(form.value) !== JSON.stringify(savedState.value))

const nameError = computed(() => form.value.name.trim().length === 0)

const I64_MIN = -(2n ** 63n)
const I64_MAX = 2n ** 63n - 1n

const seedError = computed(() => {
    if (form.value.seed === undefined || form.value.seed === savedState.value.seed) {
        return false
    }
    const trimmed = form.value.seed.trim()
    if (!/^[+-]?\d+$/.test(trimmed)) {
        return true
    }
    const value = BigInt(trimmed)
    return value < I64_MIN || value > I64_MAX
})

const invalidRules = computed(() =>
    Object.entries(form.value.rules)
        .filter(([key, value]) => {
            if (value === savedState.value.rules[key]) {
                return false
            }
            const widget = resolveGameRuleType(savedState.value.rules[key] ?? value)
            return widget === 'integer' && !/^[+-]?\d+$/.test(value.trim())
        })
        .map(([key]) => key),
)

const canSave = computed(
    () =>
        dirty.value &&
        !saving.value &&
        !nameError.value &&
        !seedError.value &&
        invalidRules.value.length === 0,
)

type RuleRow = {
    key: string
    label: string
    widget: 'boolean' | 'integer' | 'text'
    modifiedFromDefault: boolean
    canResetToDefault: boolean
    defaultValue?: string
}

const ruleGroups = computed(() => {
    if (!data.value) {
        return []
    }
    const query = ruleSearch.value.trim().toLowerCase()
    const grouped = new Map<string, RuleRow[]>()

    for (const entry of data.value.game_rules) {
        const meta = getGameRuleMetadata(entry.key)
        const label = meta ? formatMessage(meta.name) : entry.key
        if (
            query &&
            !label.toLowerCase().includes(query) &&
            !entry.key.toLowerCase().includes(query)
        ) {
            continue
        }
        const currentValue = form.value.rules[entry.key] ?? entry.value
        const category = meta?.category ?? 'other'
        const row: RuleRow = {
            key: entry.key,
            label,
            widget: resolveGameRuleType(entry.value),
            modifiedFromDefault:
                meta?.defaultValue !== undefined && currentValue !== meta.defaultValue,
            canResetToDefault:
                meta?.defaultValue !== undefined && currentValue !== meta.defaultValue,
            defaultValue: meta?.defaultValue,
        }
        const rows = grouped.get(category)
        if (rows) {
            rows.push(row)
        } else {
            grouped.set(category, [row])
        }
    }

    return RULE_CATEGORY_ORDER.filter((category) => grouped.has(category)).map((category) => ({
        category,
        label: formatMessage(gameRuleCategoryMessages[category]),
        rules: grouped.get(category)!,
    }))
})

function gameModeLabel(mode: SingleplayerGameMode) {
    return formatMessage(GAME_MODES[mode].message)
}

function difficultyLabel(difficulty: WorldDifficulty) {
    return formatMessage(messages[`difficulty_${difficulty}`])
}

function booleanLabel(value: string) {
    return formatMessage(value === 'true' ? messages.ruleEnabled : messages.ruleDisabled)
}

function buildPatch(): WorldSettingsPatch {
    const patch: WorldSettingsPatch = {}
    const current = form.value
    const saved = savedState.value

    if (current.name.trim() !== saved.name) {
        patch.name = current.name.trim()
    }
    if (current.gameMode !== saved.gameMode) {
        patch.game_mode = current.gameMode
    }
    if (current.difficulty && current.difficulty !== saved.difficulty) {
        patch.difficulty = current.difficulty
    }
    if (current.allowCommands !== undefined && current.allowCommands !== saved.allowCommands) {
        patch.allow_commands = current.allowCommands === 'true'
    }
    if (current.seed !== undefined && current.seed.trim() !== saved.seed) {
        patch.seed = current.seed.trim()
    }
    const changedRules = Object.entries(current.rules)
        .filter(([key, value]) => value !== saved.rules[key])
        .map(([key, value]) => ({ key, value: value.trim() }))
    if (changedRules.length > 0) {
        patch.game_rules = changedRules
    }
    return patch
}

async function save() {
    if (!canSave.value || !data.value) {
        return
    }
    saving.value = true
    try {
        const patch = buildPatch()
        if (Object.keys(patch).length > 0) {
            await update_world_settings(props.instance.id, worldPath.value, patch)
        }
        if (form.value.removeIcon && data.value.icon) {
            await reset_world_icon(props.instance.id, worldPath.value)
        }
        await load()
        await queryClient.invalidateQueries({ queryKey: ['worlds', props.instance.id] })
        addNotification({
            title: formatMessage(messages.savedNotification),
            type: 'success',
        })
    } catch (err) {
        handleError(err as Error)
    } finally {
        saving.value = false
    }
}

function discard() {
    form.value = JSON.parse(JSON.stringify(savedState.value))
}

function resetRuleToDefault(key: string, defaultValue?: string) {
    if (defaultValue !== undefined) {
        form.value.rules[key] = defaultValue
    }
}

const unsavedModal = ref<InstanceType<typeof NewModal>>()
let allowLeave = false
let pendingNavigation: RouteLocationNormalized | null = null

onBeforeRouteLeave((to) => {
    if (!dirty.value || readonly.value || allowLeave) {
        return true
    }
    pendingNavigation = to
    unsavedModal.value?.show()
    return false
})

function confirmLeave() {
    allowLeave = true
    unsavedModal.value?.hide()
    if (pendingNavigation) {
        router.push(pendingNavigation.fullPath)
    }
}

const messages = defineMessages({
    loadErrorHeading: {
        id: 'app.world-editor.load-error',
        defaultMessage: 'Failed to load world',
    },
    moddedBadge: {
        id: 'app.world-editor.badge.modded',
        defaultMessage: 'Modded',
    },
    hardcoreBadge: {
        id: 'app.world-editor.badge.hardcore',
        defaultMessage: 'Hardcore',
    },
    lastPlayed: {
        id: 'app.world-editor.last-played',
        defaultMessage: 'Last played {ago}',
    },
    lockedHeading: {
        id: 'app.world-editor.locked.heading',
        defaultMessage: 'World is in use',
    },
    lockedBody: {
        id: 'app.world-editor.locked.body',
        defaultMessage:
            'This world is currently open in Minecraft. Close the world before editing it — the editor is read-only until then.',
    },
    basicSection: {
        id: 'app.world-editor.section.basic',
        defaultMessage: 'Basic settings',
    },
    nameLabel: {
        id: 'app.world-editor.name.label',
        defaultMessage: 'Name',
    },
    namePlaceholder: {
        id: 'app.world-editor.name.placeholder',
        defaultMessage: 'Minecraft World',
    },
    nameRequired: {
        id: 'app.world-editor.name.required',
        defaultMessage: 'The world name cannot be empty',
    },
    gameModeLabel: {
        id: 'app.world-editor.game-mode.label',
        defaultMessage: 'Game mode',
    },
    difficultyLabel: {
        id: 'app.world-editor.difficulty.label',
        defaultMessage: 'Difficulty',
    },
    difficultyLockedHint: {
        id: 'app.world-editor.difficulty.locked-hint',
        defaultMessage: '(locked in game)',
    },
    difficulty_peaceful: {
        id: 'app.world-editor.difficulty.peaceful',
        defaultMessage: 'Peaceful',
    },
    difficulty_easy: {
        id: 'app.world-editor.difficulty.easy',
        defaultMessage: 'Easy',
    },
    difficulty_normal: {
        id: 'app.world-editor.difficulty.normal',
        defaultMessage: 'Normal',
    },
    difficulty_hard: {
        id: 'app.world-editor.difficulty.hard',
        defaultMessage: 'Hard',
    },
    allowCommandsLabel: {
        id: 'app.world-editor.allow-commands.label',
        defaultMessage: 'Allow cheats',
    },
    seedSection: {
        id: 'app.world-editor.section.seed',
        defaultMessage: 'World seed',
    },
    seedWarningHeading: {
        id: 'app.world-editor.seed.warning-heading',
        defaultMessage: 'Changing the seed only affects new terrain',
    },
    seedWarningBody: {
        id: 'app.world-editor.seed.warning-body',
        defaultMessage:
            'Chunks that have already been generated will not be regenerated, which can create visible borders between old and new terrain.',
    },
    seedInvalid: {
        id: 'app.world-editor.seed.invalid',
        defaultMessage: 'The seed must be a whole number in the 64-bit integer range',
    },
    gameRulesSection: {
        id: 'app.world-editor.section.game-rules',
        defaultMessage: 'Game rules',
    },
    searchRulesPlaceholder: {
        id: 'app.world-editor.game-rules.search-placeholder',
        defaultMessage: 'Search {count} game rules...',
    },
    noRulesFound: {
        id: 'app.world-editor.game-rules.no-results',
        defaultMessage: 'No game rules match your search',
    },
    modifiedFromDefault: {
        id: 'app.world-editor.game-rules.modified',
        defaultMessage: 'Differs from the vanilla default',
    },
    resetRuleToDefault: {
        id: 'app.world-editor.game-rules.reset-to-default',
        defaultMessage: 'Reset to default',
    },
    ruleEnabled: {
        id: 'app.world-editor.game-rules.enabled',
        defaultMessage: 'Enabled',
    },
    ruleDisabled: {
        id: 'app.world-editor.game-rules.disabled',
        defaultMessage: 'Disabled',
    },
    ruleInvalidInteger: {
        id: 'app.world-editor.game-rules.invalid-integer',
        defaultMessage: 'This rule requires a whole number',
    },
    resetIcon: {
        id: 'app.world-editor.reset-icon',
        defaultMessage: 'Reset icon',
    },
    unsavedChangesLabel: {
        id: 'app.world-editor.unsaved-changes',
        defaultMessage: 'You have unsaved changes',
    },
    discardButton: {
        id: 'app.world-editor.discard',
        defaultMessage: 'Discard changes',
    },
    savedNotification: {
        id: 'app.world-editor.saved',
        defaultMessage: 'World settings saved',
    },
    unsavedTitle: {
        id: 'app.world-editor.unsaved-modal.title',
        defaultMessage: 'Discard unsaved changes?',
    },
    unsavedBody: {
        id: 'app.world-editor.unsaved-modal.body',
        defaultMessage:
            'Your changes to this world have not been saved and will be lost if you leave.',
    },
    leaveButton: {
        id: 'app.world-editor.unsaved-modal.leave',
        defaultMessage: 'Discard and leave',
    },
    stayButton: {
        id: 'app.world-editor.unsaved-modal.stay',
        defaultMessage: 'Keep editing',
    },
})
</script>
