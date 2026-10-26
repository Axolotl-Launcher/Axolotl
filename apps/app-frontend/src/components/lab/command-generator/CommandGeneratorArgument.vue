<script setup lang="ts">
import { BoxIcon } from '@modrinth/assets'
import {
    Button,
    Combobox,
    type ComboboxOption,
    commonMessages,
    defineMessages,
    StyledInput,
    Toggle,
    useVIntl,
} from '@modrinth/ui'
import { computed, ref, useId, useTemplateRef, watch } from 'vue'

import RecipeItemIcon from '@/components/lab/recipe-generator/RecipeItemIcon.vue'
import { getCurrentVersion } from '@/lab/command-generator/commands/registry.ts'
import type { ArgumentPreset, CommandArgument } from '@/lab/command-generator/commands/types.ts'
import advancementsData from '@/lab/command-generator/data/advancements.json'
import effectsData from '@/lab/command-generator/data/effects.json'
import type { SlotDisplay } from '@/lab/recipe-generator/display'
import { getSlotDisplay } from '@/lab/recipe-generator/display'
import { buildSlotContext } from '@/lab/recipe-generator/slot-context'
import type { JavaVersionId, SlotValue } from '@/lab/recipe-generator/types'
import { loadVersionResources, TEXTURE_ATLAS } from '@/lab/shared/minecraft/resources'

import CommandGeneratorItemPickerModal from './CommandGeneratorItemPickerModal.vue'
import CommandGeneratorNbtEditor from './CommandGeneratorNbtEditor.vue'

const props = defineProps<{
    arg: CommandArgument
    modelValue: string | number | boolean | undefined
    version?: string
    forceDataComponents?: boolean
}>()

const fieldId = useId()

const emit = defineEmits<{
    'update:modelValue': [value: string | number | boolean]
}>()

const { formatMessage } = useVIntl()

const messages = defineMessages({
    argRequired: {
        id: 'app.lab.command-generator.arg.required',
        defaultMessage: 'Required',
    },
    argSelectPlaceholder: {
        id: 'app.lab.command-generator.arg.select-placeholder',
        defaultMessage: 'Select…',
    },
    coordinateRelative: {
        id: 'app.lab.command-generator.coordinate.relative',
        defaultMessage: 'Relative (~)',
    },
    coordinateLocal: {
        id: 'app.lab.command-generator.coordinate.local',
        defaultMessage: 'Local (^)',
    },
    coordinateAbsolute: {
        id: 'app.lab.command-generator.coordinate.absolute',
        defaultMessage: 'Absolute',
    },
    coordinateAxisLabel: {
        id: 'app.lab.command-generator.coordinate.axis-label',
        defaultMessage: '{axis} coordinate',
    },
    coordinateUseCoords: {
        id: 'app.lab.command-generator.coordinate.use-coords',
        defaultMessage: 'Coordinates',
    },
    coordinateUseEntity: {
        id: 'app.lab.command-generator.coordinate.use-entity',
        defaultMessage: 'Entity',
    },
    pickItem: {
        id: 'app.lab.command-generator.item-picker.button',
        defaultMessage: 'Pick item',
    },
    pickBlock: {
        id: 'app.lab.command-generator.block-picker.button',
        defaultMessage: 'Pick block',
    },
    pickEffect: {
        id: 'app.lab.command-generator.effect-picker.button',
        defaultMessage: 'Pick effect',
    },
    pickAdvancement: {
        id: 'app.lab.command-generator.advancement-picker.button',
        defaultMessage: 'Pick advancement',
    },
})

/** 数据文件里的动态 key（preset.labelKey、arg.displayNameKey、arg.descriptionKey）。 */
function formatDynamic(id: string, fallback = ''): string {
    return formatMessage({ id, defaultMessage: fallback || id })
}

const ENTITY_SELECTOR_DEFAULT_PRESETS: ArgumentPreset[] = [
    { labelKey: 'app.lab.command-generator.preset.entity.self', value: '@s' },
    { labelKey: 'app.lab.command-generator.preset.entity.nearest-player', value: '@p' },
    { labelKey: 'app.lab.command-generator.preset.entity.all-players', value: '@a' },
    { labelKey: 'app.lab.command-generator.preset.entity.random-player', value: '@r' },
    { labelKey: 'app.lab.command-generator.preset.entity.all-entities', value: '@e' },
    { labelKey: 'app.lab.command-generator.preset.entity.nearest-entity', value: '@n' },
    { labelKey: 'app.lab.command-generator.preset.entity.zombies', value: '@e[type=zombie]' },
    { labelKey: 'app.lab.command-generator.preset.entity.skeletons', value: '@e[type=skeleton]' },
    { labelKey: 'app.lab.command-generator.preset.entity.creepers', value: '@e[type=creeper]' },
    { labelKey: 'app.lab.command-generator.preset.entity.endermen', value: '@e[type=enderman]' },
    { labelKey: 'app.lab.command-generator.preset.entity.villagers', value: '@e[type=villager]' },
    { labelKey: 'app.lab.command-generator.preset.entity.items', value: '@e[type=item]' },
    {
        labelKey: 'app.lab.command-generator.preset.entity.creative-players',
        value: '@a[gamemode=creative]',
    },
    {
        labelKey: 'app.lab.command-generator.preset.entity.survival-players',
        value: '@a[gamemode=survival]',
    },
    { labelKey: 'app.lab.command-generator.preset.entity.nearby-10', value: '@p[distance=..10]' },
    {
        labelKey: 'app.lab.command-generator.preset.entity.nearby-entities-5',
        value: '@e[distance=..5]',
    },
]

const ADVANCEMENT_DEFAULT_PRESETS: ArgumentPreset[] = [
    {
        labelKey: 'app.lab.command-generator.preset.advancement.everything',
        value: 'everything',
        descriptionKey: 'app.lab.command-generator.preset.advancement.everything.desc',
    },
    {
        labelKey: 'app.lab.command-generator.preset.advancement.root',
        value: 'minecraft:story/root',
    },
    {
        labelKey: 'app.lab.command-generator.preset.advancement.stone-age',
        value: 'minecraft:story/mine_stone',
    },
    {
        labelKey: 'app.lab.command-generator.preset.advancement.diamonds',
        value: 'minecraft:story/mine_diamond',
    },
    {
        labelKey: 'app.lab.command-generator.preset.advancement.enter-nether',
        value: 'minecraft:story/enter_the_nether',
    },
    {
        labelKey: 'app.lab.command-generator.preset.advancement.kill-dragon',
        value: 'minecraft:end/kill_dragon',
    },
    {
        labelKey: 'app.lab.command-generator.preset.advancement.elytra',
        value: 'minecraft:end/elytra',
    },
    {
        labelKey: 'app.lab.command-generator.preset.advancement.hero-village',
        value: 'minecraft:adventure/hero_of_the_village',
    },
    {
        labelKey: 'app.lab.command-generator.preset.advancement.breed-animals',
        value: 'minecraft:husbandry/breed_an_animal',
    },
]

const GAMERULE_DEFAULT_PRESETS: ArgumentPreset[] = [
    {
        labelKey: 'app.lab.command-generator.preset.gamerule.keep-inventory',
        value: 'keepInventory',
    },
    { labelKey: 'app.lab.command-generator.preset.gamerule.no-mob-griefing', value: 'mobGriefing' },
    {
        labelKey: 'app.lab.command-generator.preset.gamerule.no-daylight-cycle',
        value: 'doDaylightCycle',
    },
    {
        labelKey: 'app.lab.command-generator.preset.gamerule.no-mob-spawning',
        value: 'doMobSpawning',
    },
    { labelKey: 'app.lab.command-generator.preset.gamerule.no-fire-tick', value: 'doFireTick' },
    {
        labelKey: 'app.lab.command-generator.preset.gamerule.no-weather-cycle',
        value: 'doWeatherCycle',
    },
    {
        labelKey: 'app.lab.command-generator.preset.gamerule.no-natural-regen',
        value: 'naturalRegeneration',
    },
    { labelKey: 'app.lab.command-generator.preset.gamerule.no-insomnia', value: 'doInsomnia' },
    {
        labelKey: 'app.lab.command-generator.preset.gamerule.no-command-feedback',
        value: 'sendCommandFeedback',
    },
]

const isNumber = computed(() => props.arg.type === 'number' || props.arg.type === 'integer')
const isNbt = computed(() => props.arg.supportsNbtEditor === true)
const isItem = computed(() => props.arg.type === 'item')
const isBlock = computed(() => props.arg.type === 'block')
const isEffect = computed(() => props.arg.type === 'effect')
const isAdvancement = computed(() => props.arg.type === 'advancement')

const isCoordinates = computed(
    () => props.arg.type === 'coordinates' || props.arg.coordinatesOptionalTarget === true,
)
const isMixedCoordinates = computed(() => props.arg.coordinatesOptionalTarget === true)

const hasInlinePresets = computed(
    () =>
        props.arg.type === 'entity-selector' ||
        props.arg.type === 'advancement' ||
        props.arg.type === 'gamerule',
)

const isEnumDropdown = computed(
    () => !hasInlinePresets.value && !!props.arg.enumValues && props.arg.enumValues.length > 0,
)

const enumOptions = computed<ComboboxOption<string>[]>(() =>
    (props.arg.enumValues ?? []).map((value) => ({ value, label: value })),
)

const itemPickerRef =
    useTemplateRef<InstanceType<typeof CommandGeneratorItemPickerModal>>('itemPickerRef')

const mergedPresets = computed<ArgumentPreset[]>(() => {
    const custom = props.arg.presets ?? []

    const builtin =
        props.arg.type === 'entity-selector'
            ? ENTITY_SELECTOR_DEFAULT_PRESETS
            : props.arg.type === 'advancement'
              ? ADVANCEMENT_DEFAULT_PRESETS
              : props.arg.type === 'gamerule' &&
                  Number((props.version ?? getCurrentVersion()).split('.')[0]) < 26
                ? GAMERULE_DEFAULT_PRESETS
                : null

    if (!builtin) return custom

    const seen = new Set<string>()
    const result: ArgumentPreset[] = []

    for (const p of custom) {
        const key = String(p.value)
        if (!seen.has(key)) {
            seen.add(key)
            result.push(p)
        }
    }

    for (const p of builtin) {
        const key = String(p.value)
        if (!seen.has(key)) {
            seen.add(key)
            result.push(p)
        }
    }

    return result
})

const showPresets = computed(
    () =>
        !isEnumDropdown.value &&
        !isNbt.value &&
        !isItem.value &&
        !isBlock.value &&
        !isEffect.value &&
        !isAdvancement.value &&
        !isCoordinates.value &&
        mergedPresets.value.length > 0,
)

const VISIBLE_PRESET_COUNT = 4
const showAllPresets = ref(false)
const visiblePresets = computed(() => {
    const list = mergedPresets.value
    if (showAllPresets.value || list.length <= VISIBLE_PRESET_COUNT) return list
    return list.slice(0, VISIBLE_PRESET_COUNT)
})
const hiddenPresetCount = computed(() => {
    const list = mergedPresets.value
    return list.length > VISIBLE_PRESET_COUNT ? list.length - VISIBLE_PRESET_COUNT : 0
})

function onEnumChange(value: unknown) {
    emit('update:modelValue', String(value))
}

function onTextInput(value: string) {
    if (isNumber.value) {
        emit('update:modelValue', value === '' ? '' : Number(value))
    } else {
        emit('update:modelValue', value)
    }
}

function onCheckboxChange(checked: boolean | undefined) {
    emit('update:modelValue', checked === true)
}

function applyPreset(value: string | number | boolean) {
    emit('update:modelValue', value)
}

function openItemPicker() {
    itemPickerRef.value?.show()
}

function onItemPicked(id: string) {
    emit('update:modelValue', id)
}

// ===== 物品/方块/效果/成就 图标实时渲染 =====
const inputDisplay = ref<SlotDisplay | undefined>(undefined)
let itemCtx: ReturnType<typeof buildSlotContext> | null = null
let resourcesLoaded = false
let requestId = 0

const FORCE_MISSING_ICONS = new Set([
    'minecraft:air',
    'minecraft:cave_air',
    'minecraft:void_air',
    'minecraft:water',
    'minecraft:lava',
    'minecraft:fire',
    'minecraft:nether_portal',
    'minecraft:end_portal',
    'minecraft:end_gateway',
    'minecraft:bubble_column',
    'minecraft:moving_piston',
    'minecraft:piston_head',
    'minecraft:light',
    'minecraft:frosted_ice',
])

watch(
    () => [props.modelValue, props.arg.type],
    async ([val, type]) => {
        const currentRequest = ++requestId
        if (!['item', 'block', 'effect', 'advancement'].includes(type as string)) return

        const id = String(val ?? '').trim()
        if (!id) {
            inputDisplay.value = undefined
            return
        }

        if (type === 'effect') {
            const shortId = id.split(':').pop()
            const effectData = effectsData.find((e) => e.id === shortId)

            if (!resourcesLoaded) {
                const version = (props.version || getCurrentVersion()) as JavaVersionId
                const resources = await loadVersionResources(version)
                if (currentRequest !== requestId) return
                itemCtx = buildSlotContext([], [], resources)
                resourcesLoaded = true
            }

            const iconItem = effectData?.icon || 'potion'
            let display = getSlotDisplay(
                { kind: 'item', id: `minecraft:${iconItem}` } as unknown as SlotValue,
                itemCtx!,
            )
            if (!display)
                display = getSlotDisplay(
                    { kind: 'item', id: 'minecraft:potion' } as unknown as SlotValue,
                    itemCtx!,
                )
            if (currentRequest === requestId) inputDisplay.value = display || undefined
            return
        }

        if (type === 'advancement') {
            const cleanId = id.replace('minecraft:', '')
            const adv = advancementsData.find((a) => a.id === cleanId)

            if (!resourcesLoaded) {
                const version = (props.version || getCurrentVersion()) as JavaVersionId
                const resources = await loadVersionResources(version)
                if (currentRequest !== requestId) return
                itemCtx = buildSlotContext([], [], resources)
                resourcesLoaded = true
            }

            const iconItem = adv?.icon || 'book'
            let display = getSlotDisplay(
                { kind: 'item', id: `minecraft:${iconItem}` } as unknown as SlotValue,
                itemCtx!,
            )
            if (!display)
                display = getSlotDisplay(
                    { kind: 'item', id: 'minecraft:book' } as unknown as SlotValue,
                    itemCtx!,
                )
            if (currentRequest === requestId) inputDisplay.value = display || undefined
            return
        }

        if (FORCE_MISSING_ICONS.has(id) || !id.includes('minecraft:')) {
            inputDisplay.value = undefined
            return
        }

        if (!resourcesLoaded) {
            try {
                const version = (props.version || getCurrentVersion()) as JavaVersionId
                const resources = await loadVersionResources(version)
                if (currentRequest !== requestId) return
                itemCtx = buildSlotContext([], [], resources)
                resourcesLoaded = true
            } catch {
                if (currentRequest === requestId) inputDisplay.value = undefined
                return
            }
        }

        let display = getSlotDisplay({ kind: 'item', id }, itemCtx!)
        if (!display) {
            display = getSlotDisplay({ kind: 'block', id } as unknown as SlotValue, itemCtx!)
        }
        if (currentRequest === requestId) inputDisplay.value = display || undefined
    },
    { immediate: true },
)

// ===== 坐标编辑器 =====
type CoordinateMode = 'relative' | 'local' | 'absolute'

interface CoordinatePart {
    mode: CoordinateMode
    value: string
}

const coordinateDimensions = computed<2 | 3>(() => props.arg.coordinatesDimensions ?? 3)
const COORDINATE_AXES_3D = ['x', 'y', 'z'] as const
const COORDINATE_AXES_2D = ['x', 'z'] as const
const coordinateAxes = computed<readonly string[]>(() =>
    coordinateDimensions.value === 2 ? COORDINATE_AXES_2D : COORDINATE_AXES_3D,
)

function isCoordinateString(value: string): boolean {
    return /^[-+~^0-9.\s]*$/.test(value)
}

function isCoordinateTuple(value: string): boolean {
    const parts = value.trim().split(/\s+/)
    return (
        parts.length === coordinateDimensions.value &&
        parts.every((part) => /^(?:~[-+]?\d*\.?\d*|\^[-+]?\d*\.?\d*|[-+]?\d*\.?\d+)$/.test(part))
    )
}

const mixedAsEntity = ref(
    isMixedCoordinates.value && !isCoordinateString(String(props.modelValue ?? '')),
)

function parseCoordinates(raw: string, dims: number): CoordinatePart[] {
    const parts = raw
        .trim()
        .split(/\s+/)
        .filter((p) => p.length > 0)
    const result: CoordinatePart[] = []
    for (let i = 0; i < dims; i++) {
        const part = parts[i] ?? ''
        if (!part || part === '~') {
            result.push({ mode: 'relative', value: '' })
        } else if (part.startsWith('~')) {
            result.push({ mode: 'relative', value: part.slice(1) })
        } else if (part === '^') {
            result.push({ mode: 'local', value: '' })
        } else if (part.startsWith('^')) {
            result.push({ mode: 'local', value: part.slice(1) })
        } else {
            result.push({ mode: 'absolute', value: part })
        }
    }
    return result
}

function serializeCoordinates(parts: CoordinatePart[]): string {
    return parts
        .map((part) => {
            if (part.mode === 'relative') return `~${part.value}`
            if (part.mode === 'local') return `^${part.value}`
            return part.value
        })
        .join(' ')
}

const coordinateParts = ref<CoordinatePart[]>(
    isCoordinates.value
        ? parseCoordinates(String(props.modelValue ?? ''), coordinateDimensions.value)
        : [],
)

if (
    isCoordinates.value &&
    props.arg.required &&
    !mixedAsEntity.value &&
    (props.modelValue === undefined || props.modelValue === '')
) {
    emit('update:modelValue', serializeCoordinates(coordinateParts.value))
}

watch(
    [() => props.modelValue, isCoordinates, isMixedCoordinates, coordinateDimensions],
    ([val]) => {
        if (!isCoordinates.value) return
        const next = String(val ?? '')
        if (isMixedCoordinates.value) {
            if (isCoordinateTuple(next)) mixedAsEntity.value = false
            else if (!isCoordinateString(next)) mixedAsEntity.value = true
        }
        if (mixedAsEntity.value) return
        const current = serializeCoordinates(coordinateParts.value)
        if (next === current && coordinateParts.value.length === coordinateDimensions.value) return
        coordinateParts.value = parseCoordinates(next, coordinateDimensions.value)
    },
)

function emitCoordinates() {
    if (mixedAsEntity.value) return
    emit('update:modelValue', serializeCoordinates(coordinateParts.value))
}

function cycleCoordinateMode(index: number) {
    const part = coordinateParts.value[index]
    if (!part) return

    let nextMode: CoordinateMode
    if (part.mode === 'relative') nextMode = 'absolute'
    else if (part.mode === 'absolute' && coordinateDimensions.value === 3) nextMode = 'local'
    else nextMode = 'relative'

    if (nextMode === 'local') {
        coordinateParts.value.forEach((p) => {
            p.mode = 'local'
            if (p.value === '') p.value = '0'
        })
    } else {
        part.mode = nextMode
        if (part.mode === 'absolute' && part.value === '') {
            part.value = '0'
        }
        coordinateParts.value.forEach((p, i) => {
            if (i !== index && p.mode === 'local') {
                p.mode = 'relative'
            }
        })
    }
    emitCoordinates()
}

function setCoordinateValue(index: number, value: string) {
    const part = coordinateParts.value[index]
    if (!part) return
    part.value = value.replace(/[^-0-9.]/g, '')
    emitCoordinates()
}

function clearCoordinates() {
    coordinateParts.value = parseCoordinates('', coordinateDimensions.value)
    emit('update:modelValue', '')
}

function switchMixedMode(toEntity: boolean) {
    mixedAsEntity.value = toEntity
    const val = String(props.modelValue ?? '')
    const isCoordString = isCoordinateString(val)

    if (toEntity) {
        if (!val || isCoordString) emit('update:modelValue', '@p')
    } else {
        if (val && !isCoordString) {
            coordinateParts.value = parseCoordinates('', coordinateDimensions.value)
        }
        emitCoordinates()
    }
}

function coordinateModeSymbol(mode: CoordinateMode): string {
    if (mode === 'relative') return '~'
    if (mode === 'local') return '^'
    return '='
}

function coordinateModeLabel(mode: CoordinateMode): string {
    if (mode === 'relative') return formatMessage(messages.coordinateRelative)
    if (mode === 'local') return formatMessage(messages.coordinateLocal)
    return formatMessage(messages.coordinateAbsolute)
}
</script>

<template>
    <div
        class="argument-card flex min-w-0 flex-col gap-2"
        :class="{ 'md:col-span-2': isNbt }"
        :data-testid="`arg-card-${arg.name}`"
    >
        <div class="flex flex-wrap items-center justify-between gap-2">
            <div class="flex flex-wrap items-center gap-2">
                <label
                    :for="fieldId"
                    class="text-sm font-semibold text-[var(--color-text-primary)]"
                    >{{ formatDynamic(arg.displayNameKey, arg.name) }}</label
                >
                <span v-if="arg.required" class="text-xs text-[var(--color-text-tertiary)]">{{
                    formatMessage(messages.argRequired)
                }}</span>
            </div>
            <div v-if="isMixedCoordinates" class="flex items-center gap-1">
                <button
                    type="button"
                    class="rounded-md px-2 py-0.5 text-xs transition-colors"
                    :class="
                        !mixedAsEntity
                            ? 'bg-surface-4 text-[var(--color-text-primary)] font-medium'
                            : 'text-[var(--color-text-tertiary)] hover:text-[var(--color-text-primary)]'
                    "
                    @click="switchMixedMode(false)"
                >
                    {{ formatMessage(messages.coordinateUseCoords) }}
                </button>
                <button
                    type="button"
                    class="rounded-md px-2 py-0.5 text-xs transition-colors"
                    :class="
                        mixedAsEntity
                            ? 'bg-surface-4 text-[var(--color-text-primary)] font-medium'
                            : 'text-[var(--color-text-tertiary)] hover:text-[var(--color-text-primary)]'
                    "
                    @click="switchMixedMode(true)"
                >
                    {{ formatMessage(messages.coordinateUseEntity) }}
                </button>
            </div>
        </div>
        <CommandGeneratorNbtEditor
            v-if="isNbt"
            :model-value="String(modelValue ?? '')"
            :kind="arg.nbtKind"
            :version="version"
            :force-data-components="forceDataComponents"
            @update:model-value="emit('update:modelValue', $event)"
        />
        <Combobox
            v-else-if="isEnumDropdown"
            :model-value="String(modelValue ?? '')"
            :options="enumOptions"
            :aria-label="formatDynamic(arg.displayNameKey, arg.name)"
            :placeholder="formatMessage(messages.argSelectPlaceholder)"
            width="full"
            @update:model-value="onEnumChange"
        />
        <Toggle
            v-else-if="arg.type === 'boolean'"
            :id="fieldId"
            :model-value="modelValue === true || modelValue === 'true'"
            :aria-label="formatDynamic(arg.displayNameKey, arg.name)"
            @update:model-value="onCheckboxChange"
        />
        <div
            v-else-if="isItem || isBlock || isEffect || isAdvancement"
            class="flex min-w-0 flex-wrap gap-2"
        >
            <div class="relative min-w-0 flex-1">
                <div class="pointer-events-none absolute inset-y-0 left-0 flex items-center pl-3">
                    <RecipeItemIcon
                        v-if="inputDisplay"
                        :display="inputDisplay"
                        :atlas="TEXTURE_ATLAS"
                        :size="20"
                        :show-count="false"
                    />
                    <BoxIcon v-else class="h-5 w-5 text-[var(--color-text-tertiary)]" />
                </div>
                <StyledInput
                    :id="fieldId"
                    :model-value="String(modelValue ?? '')"
                    :placeholder="arg.placeholder ?? formatDynamic(arg.descriptionKey)"
                    class="w-full pl-10"
                    :spellcheck="false"
                    @update:model-value="onTextInput(String($event ?? ''))"
                />
            </div>
            <Button type="outlined" @click="openItemPicker">
                {{
                    formatMessage(
                        isBlock
                            ? messages.pickBlock
                            : isEffect
                              ? messages.pickEffect
                              : isAdvancement
                                ? messages.pickAdvancement
                                : messages.pickItem,
                    )
                }}
            </Button>
        </div>
        <StyledInput
            v-else-if="isMixedCoordinates && mixedAsEntity"
            :id="fieldId"
            :model-value="String(modelValue ?? '')"
            :placeholder="arg.placeholder ?? formatDynamic(arg.descriptionKey)"
            class="w-full"
            :spellcheck="false"
            @update:model-value="onTextInput(String($event ?? ''))"
        />
        <div v-else-if="isCoordinates" class="flex min-w-0 flex-col gap-2">
            <div class="flex min-w-0 flex-wrap items-center gap-2">
                <div
                    v-for="(part, index) in coordinateParts"
                    :key="index"
                    class="flex min-w-0 items-center gap-1"
                >
                    <button
                        v-tooltip="coordinateModeLabel(part.mode)"
                        type="button"
                        class="coordinate-mode flex shrink-0 items-center justify-center rounded-lg border border-solid border-surface-4 bg-surface-3 px-2 py-2 font-mono text-sm leading-none text-[var(--color-text-primary)] hover:border-brand focus-visible:outline-brand"
                        :aria-label="coordinateModeLabel(part.mode)"
                        @click="cycleCoordinateMode(index)"
                    >
                        {{ coordinateModeSymbol(part.mode) }}
                    </button>
                    <StyledInput
                        :model-value="part.value"
                        :aria-label="
                            formatMessage(messages.coordinateAxisLabel, {
                                axis: coordinateAxes[index] ?? '0',
                            })
                        "
                        :placeholder="
                            part.mode === 'relative' ? '0' : (coordinateAxes[index] ?? '0')
                        "
                        style="width: 4.5rem"
                        :spellcheck="false"
                        @update:model-value="setCoordinateValue(index, String($event ?? ''))"
                    />
                </div>
                <Button
                    v-if="!arg.required && modelValue !== undefined && modelValue !== ''"
                    type="quiet"
                    size="sm"
                    @click="clearCoordinates"
                >
                    {{ formatMessage(commonMessages.clearButton) }}
                </Button>
            </div>
        </div>
        <StyledInput
            v-else
            :id="fieldId"
            :model-value="String(modelValue ?? '')"
            :type="isNumber ? 'number' : 'text'"
            :placeholder="arg.placeholder ?? formatDynamic(arg.descriptionKey)"
            :step="arg.type === 'integer' ? 1 : undefined"
            :min="arg.min"
            :max="arg.max"
            class="w-full"
            :spellcheck="false"
            @update:model-value="onTextInput(String($event ?? ''))"
        />
        <p v-if="!isNbt" class="m-0 text-xs leading-5 text-[var(--color-text-tertiary)]">
            {{ formatDynamic(arg.descriptionKey) }}
        </p>
        <div v-if="showPresets" class="flex flex-wrap gap-1.5">
            <Button
                v-for="preset in visiblePresets"
                :key="preset.labelKey"
                v-tooltip="preset.descriptionKey ? formatDynamic(preset.descriptionKey) : undefined"
                type="quiet"
                size="2xs"
                @click="applyPreset(preset.value)"
                >{{ formatDynamic(preset.labelKey) }}</Button
            >
            <Button
                v-if="hiddenPresetCount > 0 && !showAllPresets"
                type="quiet"
                size="2xs"
                @click="showAllPresets = true"
                >+{{ hiddenPresetCount }}</Button
            >
        </div>
        <CommandGeneratorItemPickerModal
            v-if="isItem || isBlock || isEffect || isAdvancement"
            ref="itemPickerRef"
            :version="getCurrentVersion()"
            :kind="isBlock ? 'block' : isEffect ? 'effect' : isAdvancement ? 'advancement' : 'item'"
            @select="onItemPicked"
        />
    </div>
</template>
