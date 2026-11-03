<script setup lang="ts">
import { ChevronDownIcon, SearchIcon } from '@modrinth/assets'
import { defineMessages, StyledInput, useVIntl } from '@modrinth/ui'
import { computed, ref, watch } from 'vue'

import BannerPreview from '@/components/lab/banner-designer/BannerPreview.vue'
import { presetNameMessages, presetSectionMessages } from '@/lab/banner-designer/messages.ts'
import {
    BANNER_PRESETS,
    type BannerPreset,
    type BannerPresetSection,
    presetsInSection,
} from '@/lab/banner-designer/presets.ts'
import type { BannerDesign } from '@/lab/banner-designer/types.ts'

const emit = defineEmits<{
    select: [design: BannerDesign]
}>()

const { formatMessage } = useVIntl()

const messages = defineMessages({
    search: {
        id: 'app.lab.banner-designer.create.preset-search',
        defaultMessage: 'Search presets',
    },
    empty: {
        id: 'app.lab.banner-designer.create.preset-empty',
        defaultMessage: 'No preset matches your search.',
    },
})

const SECTION_ORDER: readonly BannerPresetSection[] = [
    'patterns',
    'uppercase',
    'lowercase',
    'digits',
    'suits',
]

/** Sections that start collapsed: the alphabets are long, the patterns are the usual entry point. */
const COLLAPSED_BY_DEFAULT: readonly BannerPresetSection[] = [
    'uppercase',
    'lowercase',
    'digits',
    'suits',
]

interface PresetEntry {
    preset: BannerPreset
    design: BannerDesign
    name: string
}

const query = ref('')
const collapsed = ref<BannerPresetSection[]>([...COLLAPSED_BY_DEFAULT])

/** Designs are built once: every render would otherwise hand the previews a new object. */
const designs = new Map(BANNER_PRESETS.map((preset) => [preset.id, preset.design()]))

const entries = computed(() =>
    SECTION_ORDER.map((section) => ({
        section,
        entries: presetsInSection(section).map<PresetEntry>((preset) => ({
            preset,
            design: designs.get(preset.id)!,
            name: preset.label ?? formatMessage(presetNameMessages[preset.id]),
        })),
    })),
)

const groups = computed(() => {
    const needle = query.value.trim().toLowerCase()
    return entries.value
        .map((group) => ({
            section: group.section,
            entries:
                needle === ''
                    ? group.entries
                    : group.entries.filter((entry) => entry.name.toLowerCase().includes(needle)),
        }))
        .filter((group) => group.entries.length > 0)
})

const isOpen = (section: BannerPresetSection) => !collapsed.value.includes(section)

function toggle(section: BannerPresetSection) {
    collapsed.value = isOpen(section)
        ? [...collapsed.value, section]
        : collapsed.value.filter((entry) => entry !== section)
}

/** A search should surface every match, so start from a fully expanded list. */
watch(query, (value) => {
    if (value.trim() !== '') collapsed.value = []
})
</script>

<template>
    <div class="flex flex-col gap-3">
        <StyledInput
            v-model="query"
            :icon="SearchIcon"
            clearable
            :placeholder="formatMessage(messages.search)"
        />

        <p
            v-if="groups.length === 0"
            class="m-0 rounded-[var(--radius-md)] border border-dashed border-surface-5 p-4 text-center text-xs text-[var(--color-text-tertiary)]"
        >
            {{ formatMessage(messages.empty) }}
        </p>

        <section v-for="group in groups" :key="group.section" class="flex flex-col gap-2">
            <button
                type="button"
                class="flex cursor-pointer items-center gap-1.5 border-0 bg-transparent p-0 text-left focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-brand"
                :aria-expanded="isOpen(group.section)"
                @click="toggle(group.section)"
            >
                <ChevronDownIcon
                    class="size-4 shrink-0 text-[var(--color-text-tertiary)] transition-transform"
                    :class="isOpen(group.section) ? '' : '-rotate-90'"
                />
                <span
                    class="text-xs font-semibold uppercase tracking-wide text-[var(--color-text-tertiary)]"
                >
                    {{ formatMessage(presetSectionMessages[group.section]) }}
                </span>
                <span class="text-xs tabular-nums text-[var(--color-text-tertiary)]">
                    {{ group.entries.length }}
                </span>
            </button>

            <div
                v-if="isOpen(group.section)"
                class="grid gap-2"
                :class="
                    group.section === 'patterns'
                        ? 'grid-cols-[repeat(auto-fill,minmax(5.5rem,1fr))]'
                        : 'grid-cols-[repeat(auto-fill,minmax(3.75rem,1fr))]'
                "
            >
                <button
                    v-for="entry in group.entries"
                    :key="entry.preset.id"
                    type="button"
                    class="flex min-w-0 cursor-pointer flex-col items-center gap-2 rounded-[var(--radius-md)] border border-surface-5 bg-surface-2 p-2 transition-[border-color,background-color] hover:border-brand hover:bg-surface-4 focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-brand"
                    @click="emit('select', entry.design)"
                >
                    <BannerPreview
                        :design="entry.design"
                        :scale="2"
                        :label="entry.name"
                        class="rounded-[2px]"
                    />
                    <span
                        class="line-clamp-2 text-center text-[11px] leading-tight text-[var(--color-text-tertiary)]"
                    >
                        {{ entry.name }}
                    </span>
                </button>
            </div>
        </section>
    </div>
</template>
