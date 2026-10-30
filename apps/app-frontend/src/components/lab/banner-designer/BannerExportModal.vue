<script setup lang="ts">
import { CheckIcon, ClipboardCopyIcon, SaveIcon, TriangleAlertIcon } from '@modrinth/assets'
import { Button, Chips, Combobox, defineMessages, NewModal, useVIntl } from '@modrinth/ui'
import { isTauri } from '@tauri-apps/api/core'
import { save } from '@tauri-apps/plugin-dialog'
import { writeTextFile } from '@tauri-apps/plugin-fs'
import { computed, ref, useTemplateRef, watch } from 'vue'

import BannerItemIcon from '@/components/lab/banner-designer/BannerItemIcon.vue'
import { describeBannerMaterials } from '@/lab/banner-designer/materials.ts'
import { patternNameMessages } from '@/lab/banner-designer/messages.ts'
import {
    type BannerGameVersion,
    toDesignJson,
    toGiveCommand,
    toItemData,
    unsupportedPatterns,
} from '@/lab/banner-designer/serialization.ts'
import {
    type BannerExportFormat,
    loadExportPreference,
    saveExportPreference,
} from '@/lab/banner-designer/storage.ts'
import type { BannerDesign } from '@/lab/banner-designer/types.ts'
import { resolveRecipeItemName } from '@/lab/shared/minecraft/item-names.ts'

const props = defineProps<{ design: BannerDesign }>()

const { formatMessage, locale } = useVIntl()
const modal = useTemplateRef<InstanceType<typeof NewModal>>('modal')

const messages = defineMessages({
    title: { id: 'app.lab.banner-designer.export.title', defaultMessage: 'Export banner' },
    give: { id: 'app.lab.banner-designer.export.give', defaultMessage: 'Give command' },
    data: { id: 'app.lab.banner-designer.export.data', defaultMessage: 'Item data' },
    json: { id: 'app.lab.banner-designer.export.json', defaultMessage: 'Design JSON' },
    modern: {
        id: 'app.lab.banner-designer.export.version.modern',
        defaultMessage: '1.20.5 and newer',
    },
    legacy: {
        id: 'app.lab.banner-designer.export.version.legacy',
        defaultMessage: '1.13 – 1.20.4',
    },
    classic: {
        id: 'app.lab.banner-designer.export.version.classic',
        defaultMessage: '1.12.2 and older',
    },
    version: { id: 'app.lab.banner-designer.export.version', defaultMessage: 'Game version' },
    hintModern: {
        id: 'app.lab.banner-designer.export.hint.modern',
        defaultMessage: 'Older versions reject the component syntax.',
    },
    hintLegacy: {
        id: 'app.lab.banner-designer.export.hint.legacy',
        defaultMessage: 'Short pattern codes. 1.20.5 and newer reject this item NBT.',
    },
    hintClassic: {
        id: 'app.lab.banner-designer.export.hint.classic',
        defaultMessage:
            'The single banner item, with a damage value of 0. 1.13 and newer need one item per dye.',
    },
    dropped: {
        id: 'app.lab.banner-designer.export.dropped',
        defaultMessage:
            'This version has no {patterns}. The game skips unknown patterns, so the banner arrives plain.',
    },
    noItemData: {
        id: 'app.lab.banner-designer.export.no-item-data',
        defaultMessage: 'A banner without patterns has no item data.',
    },
    copy: { id: 'app.lab.banner-designer.export.copy', defaultMessage: 'Copy' },
    copied: { id: 'app.lab.banner-designer.export.copied', defaultMessage: 'Copied' },
    copyFailed: {
        id: 'app.lab.banner-designer.export.copy-failed',
        defaultMessage: 'The text could not be copied. Select it and copy it by hand.',
    },
    saveFile: { id: 'app.lab.banner-designer.export.save-file', defaultMessage: 'Save to file' },
    saved: { id: 'app.lab.banner-designer.export.saved', defaultMessage: 'Saved' },
    saveFailed: {
        id: 'app.lab.banner-designer.export.save-failed',
        defaultMessage: 'The file could not be written.',
    },
    materials: {
        id: 'app.lab.banner-designer.export.materials',
        defaultMessage: 'Materials',
    },
    patternItems: {
        id: 'app.lab.banner-designer.export.pattern-items',
        defaultMessage: 'Banner pattern items',
    },
    patternItemNote: {
        id: 'app.lab.banner-designer.export.pattern-item-note',
        defaultMessage: 'The loom does not consume banner pattern items.',
    },
    noDyes: {
        id: 'app.lab.banner-designer.export.no-dyes',
        defaultMessage: 'A banner without patterns needs no dyes.',
    },
    close: { id: 'app.lab.banner-designer.export.close', defaultMessage: 'Close' },
})

const preference = loadExportPreference()

const format = ref<BannerExportFormat | null>(preference?.format ?? 'give')
const version = ref<BannerGameVersion | null>(preference?.version ?? 'modern')
const copied = ref(false)
const copyError = ref<string | null>(null)
const saved = ref(false)
const saveError = ref<string | null>(null)
let copyTimer: ReturnType<typeof setTimeout> | undefined
let saveTimer: ReturnType<typeof setTimeout> | undefined

const formats: BannerExportFormat[] = ['give', 'data', 'json']
const versions: BannerGameVersion[] = ['modern', 'legacy', 'classic']

const activeFormat = computed<BannerExportFormat>(() => format.value ?? 'give')
const activeVersion = computed<BannerGameVersion>(() => version.value ?? 'modern')

const materials = computed(() => describeBannerMaterials(props.design))

const missingPatterns = computed(() =>
    activeFormat.value === 'json' ? [] : unsupportedPatterns(props.design, activeVersion.value),
)
const missingPatternNames = computed(() =>
    missingPatterns.value.map((pattern) => formatMessage(patternNameMessages[pattern])).join(', '),
)

watch([activeFormat, activeVersion], ([nextFormat, nextVersion]) => {
    // A failure belongs to the text that was on screen when it happened.
    saveError.value = null
    copyError.value = null
    saveExportPreference({ format: nextFormat, version: nextVersion })
})

const code = computed(() => {
    if (activeFormat.value === 'json') return toDesignJson(props.design)
    const version = activeVersion.value
    if (activeFormat.value === 'give') return toGiveCommand(props.design, { version })
    return toItemData(props.design, { version })
})

const showVersion = computed(() => activeFormat.value !== 'json')

const versionOptions = computed(() =>
    versions.map((value) => ({ value, label: versionLabel(value) })),
)

const versionHints: Record<BannerGameVersion, (typeof messages)['hintModern']> = {
    modern: messages.hintModern,
    legacy: messages.hintLegacy,
    classic: messages.hintClassic,
}

const versionHint = computed(() => versionHints[activeVersion.value])

/**
 * Item data and design JSON are meant to be kept, so they can be written to a
 * file. A give command is pasted into the game instead.
 */
const canSaveToFile = computed(() => isTauri() && activeFormat.value !== 'give')
const fileTarget = computed(() =>
    activeFormat.value === 'json'
        ? { name: 'banner-design.json', extension: 'json', label: messages.json }
        : { name: 'banner-item-data.txt', extension: 'txt', label: messages.data },
)

function formatLabel(value: BannerExportFormat) {
    return formatMessage(messages[value])
}

function versionLabel(value: BannerGameVersion) {
    return formatMessage(messages[value])
}

function itemName(item: string) {
    return resolveRecipeItemName(item, locale.value, item.replace(/^minecraft:/, ''))
}

async function copy() {
    try {
        await navigator.clipboard.writeText(code.value)
        copyError.value = null
        copied.value = true
        clearTimeout(copyTimer)
        copyTimer = setTimeout(() => {
            copied.value = false
        }, 1500)
    } catch {
        copied.value = false
        copyError.value = formatMessage(messages.copyFailed)
    }
}

async function saveToFile() {
    if (!code.value) return
    const target = fileTarget.value
    try {
        const path = await save({
            defaultPath: target.name,
            filters: [{ name: formatMessage(target.label), extensions: [target.extension] }],
        })
        if (!path) return
        await writeTextFile(path, code.value)
        saveError.value = null
        saved.value = true
        clearTimeout(saveTimer)
        saveTimer = setTimeout(() => {
            saved.value = false
        }, 1500)
    } catch {
        saved.value = false
        saveError.value = formatMessage(messages.saveFailed)
    }
}

function show() {
    copied.value = false
    copyError.value = null
    saved.value = false
    saveError.value = null
    modal.value?.show()
}

defineExpose({ show })
</script>

<template>
    <NewModal
        ref="modal"
        :header="formatMessage(messages.title)"
        width="min(50rem, calc(95vw - 10rem))"
        max-width="50rem"
        scrollable
        max-content-height="min(44rem, 84vh)"
    >
        <div class="flex flex-col gap-4">
            <div class="flex flex-wrap items-center justify-between gap-3">
                <Chips
                    v-model="format"
                    :items="formats"
                    :format-label="formatLabel"
                    :capitalize="false"
                    :aria-label="formatMessage(messages.title)"
                />
                <div v-if="showVersion" class="flex items-center gap-2">
                    <span class="text-xs font-medium text-[var(--color-text-tertiary)]">
                        {{ formatMessage(messages.version) }}
                    </span>
                    <Combobox
                        :model-value="activeVersion"
                        :options="versionOptions"
                        width="content"
                        @update:model-value="version = $event"
                    />
                </div>
            </div>

            <div class="flex flex-col gap-2">
                <div
                    class="relative rounded-[var(--radius-md)] border border-surface-5 bg-surface-1 p-3 pr-12"
                >
                    <pre
                        class="m-0 max-h-56 overflow-auto whitespace-pre-wrap break-all font-mono text-xs leading-relaxed text-[var(--color-text-default)] select-all"
                        >{{ code || formatMessage(messages.noItemData) }}</pre>
                    <Button
                        v-tooltip="formatMessage(copied ? messages.copied : messages.copy)"
                        type="quiet"
                        size="2xs"
                        circular
                        icon-only
                        class="absolute right-2 top-2"
                        :aria-label="formatMessage(copied ? messages.copied : messages.copy)"
                        :disabled="!code"
                        @click="copy"
                    >
                        <CheckIcon v-if="copied" />
                        <ClipboardCopyIcon v-else />
                    </Button>
                </div>

                <p v-if="copyError" class="m-0 text-xs text-brand-red" role="alert">
                    {{ copyError }}
                </p>

                <p v-if="showVersion" class="m-0 text-xs text-[var(--color-text-tertiary)]">
                    {{ formatMessage(versionHint) }}
                </p>

                <p
                    v-if="missingPatterns.length"
                    class="m-0 flex items-start gap-1.5 text-xs text-orange"
                >
                    <TriangleAlertIcon class="mt-px size-3.5 shrink-0" aria-hidden="true" />
                    <span>{{
                        formatMessage(messages.dropped, { patterns: missingPatternNames })
                    }}</span>
                </p>

                <p v-if="saveError" class="m-0 text-xs text-brand-red" role="alert">
                    {{ saveError }}
                </p>
            </div>

            <section class="flex flex-col gap-2">
                <h3
                    class="m-0 text-xs font-semibold uppercase tracking-wide text-[var(--color-text-tertiary)]"
                >
                    {{ formatMessage(messages.materials) }}
                </h3>

                <ul class="m-0 flex list-none flex-col gap-1.5 p-0">
                    <li class="flex items-center gap-2 text-sm text-[var(--color-text-primary)]">
                        <BannerItemIcon :item="materials.bannerItem" :size="20" />
                        <span class="min-w-0 flex-1 truncate">{{
                            itemName(materials.bannerItem)
                        }}</span>
                        <span class="tabular-nums text-[var(--color-text-tertiary)]">× 1</span>
                    </li>
                    <li
                        v-for="dye in materials.dyes"
                        :key="dye.color"
                        class="flex items-center gap-2 text-sm text-[var(--color-text-primary)]"
                    >
                        <BannerItemIcon :item="dye.item" :size="20" />
                        <span class="min-w-0 flex-1 truncate">{{ itemName(dye.item) }}</span>
                        <span class="tabular-nums text-[var(--color-text-tertiary)]"
                            >× {{ dye.count }}</span
                        >
                    </li>
                </ul>

                <p
                    v-if="materials.dyes.length === 0"
                    class="m-0 text-xs text-[var(--color-text-tertiary)]"
                >
                    {{ formatMessage(messages.noDyes) }}
                </p>

                <template v-if="materials.patternItems.length">
                    <h3
                        class="m-0 mt-2 text-xs font-semibold uppercase tracking-wide text-[var(--color-text-tertiary)]"
                    >
                        {{ formatMessage(messages.patternItems) }}
                    </h3>
                    <ul class="m-0 flex list-none flex-col gap-1.5 p-0">
                        <li
                            v-for="item in materials.patternItems"
                            :key="item"
                            class="flex items-center gap-2 text-sm text-[var(--color-text-primary)]"
                        >
                            <BannerItemIcon :item="item" :size="20" />
                            <span class="min-w-0 flex-1 truncate">{{ itemName(item) }}</span>
                            <span class="tabular-nums text-[var(--color-text-tertiary)]">× 1</span>
                        </li>
                    </ul>
                    <p class="m-0 text-xs text-[var(--color-text-tertiary)]">
                        {{ formatMessage(messages.patternItemNote) }}
                    </p>
                </template>
            </section>
        </div>

        <template #actions>
            <div class="flex items-center justify-between gap-2">
                <Button v-if="canSaveToFile" type="outlined" :disabled="!code" @click="saveToFile">
                    <CheckIcon v-if="saved" />
                    <SaveIcon v-else />
                    {{ formatMessage(saved ? messages.saved : messages.saveFile) }}
                </Button>
                <span v-else></span>
                <Button type="quiet" @click="modal?.hide()">{{
                    formatMessage(messages.close)
                }}</Button>
            </div>
        </template>
    </NewModal>
</template>
