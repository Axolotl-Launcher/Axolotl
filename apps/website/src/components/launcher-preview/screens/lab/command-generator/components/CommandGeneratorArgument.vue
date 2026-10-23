<script setup lang="ts">
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import type { ComboboxOption } from '@modrinth/ui/src/components/base/Combobox.vue'
import Combobox from '@modrinth/ui/src/components/base/Combobox.vue'
import StyledInput from '@modrinth/ui/src/components/base/StyledInput.vue'
import Toggle from '@modrinth/ui/src/components/base/Toggle.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { computed, ref, useId, useTemplateRef } from 'vue'

import { getCurrentVersion } from '../commands/registry.ts'
import type { ArgumentPreset, CommandArgument } from '../commands/types.ts'
import { commandGeneratorMessages as messages } from '../i18n/messages.ts'
import CommandGeneratorItemPickerModal from './CommandGeneratorItemPickerModal.vue'
import CommandGeneratorNbtEditor from './CommandGeneratorNbtEditor.vue'

const props = defineProps<{
    arg: CommandArgument
    modelValue: string | number | boolean | undefined
    /** 当前选中的游戏版本，用于 NBT 序列化分支 */
    version?: string
    /**
     * 是否强制使用 1.20.5+ 数据组件格式。
     * - undefined：由 NBT 编辑器内部根据 version 自动判定
     * - true / false：仅在 1.20.x 下由用户显式切换时传入
     */
    forceDataComponents?: boolean
}>()

const fieldId = useId()

const emit = defineEmits<{
    'update:modelValue': [value: string | number | boolean]
}>()

const { formatMessage } = useVIntl()

function t(key: string, fallback?: string): string {
    return formatMessage({ id: key, defaultMessage: fallback ?? key })
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

/**
 * 这三种类型统一走「普通 input + 下方预设 chip」的路径，
 * 显式排除出 Combobox 分支，避免被 enumValues 抢占。
 */
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
    () => !isEnumDropdown.value && !isNbt.value && !isItem.value && mergedPresets.value.length > 0,
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

function onCheckboxChange(checked: boolean) {
    emit('update:modelValue', checked)
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
</script>

<template>
    <div class="argument-card flex min-w-0 flex-col gap-2" :class="{ 'md:col-span-2': isNbt }">
        <div class="flex flex-wrap items-center gap-2">
            <label :for="fieldId" class="text-sm font-semibold text-[var(--color-text-primary)]">{{
                t(arg.displayNameKey)
            }}</label>
            <span v-if="arg.required" class="text-xs text-[var(--color-text-tertiary)]">{{
                formatMessage(messages.argRequired)
            }}</span>
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
            :aria-label="t(arg.displayNameKey)"
            :placeholder="formatMessage(messages.argSelectPlaceholder)"
            width="full"
            @update:model-value="onEnumChange"
        />
        <Toggle
            v-else-if="arg.type === 'boolean'"
            :id="fieldId"
            :model-value="modelValue === true || modelValue === 'true'"
            :aria-label="t(arg.displayNameKey)"
            @update:model-value="onCheckboxChange"
        />
        <div v-else-if="isItem" class="flex min-w-0 flex-wrap gap-2">
            <StyledInput
                :id="fieldId"
                :model-value="String(modelValue ?? '')"
                :placeholder="arg.placeholder ?? t(arg.descriptionKey)"
                class="min-w-0 flex-1"
                :spellcheck="false"
                @update:model-value="onTextInput(String($event ?? ''))"
            />
            <Button type="outlined" @click="openItemPicker">{{
                t('app.lab.command-generator.item-picker.button')
            }}</Button>
        </div>
        <StyledInput
            v-else
            :id="fieldId"
            :model-value="String(modelValue ?? '')"
            :type="isNumber ? 'number' : 'text'"
            :placeholder="arg.placeholder ?? t(arg.descriptionKey)"
            :step="arg.type === 'integer' ? 1 : undefined"
            :min="arg.min"
            :max="arg.max"
            class="w-full"
            :spellcheck="false"
            @update:model-value="onTextInput(String($event ?? ''))"
        />
        <p v-if="!isNbt" class="m-0 text-xs leading-5 text-[var(--color-text-tertiary)]">
            {{ t(arg.descriptionKey) }}
        </p>
        <div v-if="showPresets" class="flex flex-wrap gap-1.5">
            <Button
                v-for="preset in visiblePresets"
                :key="preset.labelKey"
                v-tooltip="preset.descriptionKey ? t(preset.descriptionKey) : undefined"
                type="quiet"
                size="2xs"
                @click="applyPreset(preset.value)"
                >{{ t(preset.labelKey) }}</Button
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
            v-if="isItem"
            ref="itemPickerRef"
            :version="getCurrentVersion()"
            @select="onItemPicked"
        />
    </div>
</template>
