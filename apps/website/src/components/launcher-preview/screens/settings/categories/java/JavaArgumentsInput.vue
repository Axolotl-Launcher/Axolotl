<script setup lang="ts">
import CheckIcon from '@modrinth/assets/icons/check.svg?component'
import CopyIcon from '@modrinth/assets/icons/copy.svg?component'
import DropdownIcon from '@modrinth/assets/icons/dropdown.svg?component'
import ExternalIcon from '@modrinth/assets/icons/external.svg?component'
import GlobeIcon from '@modrinth/assets/icons/globe.svg?component'
import SparklesIcon from '@modrinth/assets/icons/sparkles.svg?component'
import XIcon from '@modrinth/assets/icons/x.svg?component'
import AutoLink from '@modrinth/ui/src/components/base/AutoLink.vue'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import Collapsible from '@modrinth/ui/src/components/base/Collapsible.vue'
import TagItem from '@modrinth/ui/src/components/base/TagItem.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue'

import { usePreview } from '../../../../lib/state/index'
import PreviewModal from '../../../../overlays/PreviewModal.vue'
import { messages } from '../../messages'
import {
    type GcContext,
    getJavaArgumentPresets,
    getPresetsByGroup,
    type JavaArgumentPreset,
    lastGcLaunchReport,
    resolveAutoGcArgs,
} from './gc'

/**
 * The Java arguments field with preset chips and the presets dialog
 * (apps/app-frontend/src/components/ui/JavaArgumentsInput.vue). Presets at the start of
 * the arguments show as removable tags; the rest stays editable text.
 */
const model = defineModel<string>({ required: true })

const props = withDefaults(
    defineProps<{
        id?: string
        placeholder?: string
        disabled?: boolean
        gcContext?: GcContext
        showAutoDetails?: boolean
    }>(),
    {
        id: undefined,
        placeholder: undefined,
        disabled: false,
        gcContext: undefined,
        showAutoDetails: false,
    },
)

const { formatMessage } = useVIntl()
const { clock } = usePreview()

const presets = computed(() => getJavaArgumentPresets(props.gcContext))
const groupedPresets = computed(() => getPresetsByGroup(presets.value))

const modal = ref<InstanceType<typeof PreviewModal>>()
const expandedPresetIds = ref(new Set<string>())
const expandedGroupIds = ref(new Set<string>())
const copiedPresetId = ref<string | null>(null)
let cancelCopiedReset: (() => void) | undefined

function getPresetArgs(preset: JavaArgumentPreset): string {
    return preset.resolveArgs ? preset.resolveArgs(props.gcContext) : preset.args
}

function getDisplayArgs(preset: JavaArgumentPreset): string {
    if (preset.id === 'gc-auto' && props.showAutoDetails && props.gcContext) {
        return resolveAutoGcArgs(props.gcContext)
    }
    return getPresetArgs(preset)
}

function getActivePresets(value: string): JavaArgumentPreset[] {
    const trimmed = value.trimStart()
    const matches = presets.value
        .filter((preset) =>
            preset.detect ? preset.detect(trimmed) : trimmed.startsWith(preset.args),
        )
        .sort((a, b) => {
            const aIsAuto = a.id === 'gc-auto' ? 1 : 0
            const bIsAuto = b.id === 'gc-auto' ? 1 : 0
            return aIsAuto - bIsAuto || presets.value.indexOf(a) - presets.value.indexOf(b)
        })
    const result: JavaArgumentPreset[] = []
    const seenGroups = new Set<string>()
    for (const preset of matches) {
        if (seenGroups.has(preset.group)) continue
        seenGroups.add(preset.group)
        result.push(preset)
    }
    return result
}

function removePresetArgs(preset: JavaArgumentPreset, args: string): string {
    const presetArgs = getPresetArgs(preset)
    if (args.startsWith(presetArgs)) {
        return args.slice(presetArgs.length).trimStart()
    }
    if (preset.detect && preset.detect(args)) {
        return args.replace(presetArgs, '').trimStart()
    }
    return args
}

const split = computed(() => {
    const trimmed = model.value.trimStart()
    const active = getActivePresets(trimmed)
    let rest = trimmed
    for (const preset of active) {
        rest = removePresetArgs(preset, rest)
    }
    return { active, rest }
})

const activePresets = computed(() => split.value.active)

const rest = computed<string>({
    get: () => split.value.rest,
    set: (value) => {
        const argsToJoin = activePresets.value.map(getPresetArgs)
        model.value = argsToJoin.length ? argsToJoin.join(' ') + (value ? ` ${value}` : '') : value
    },
})

function onInput(event: Event) {
    rest.value = (event.target as HTMLInputElement).value
}

function removeOtherGroupPresets(preset: JavaArgumentPreset, currentArgs: string): string {
    const groupPresets = presets.value.filter((p) => p.group === preset.group && p.id !== preset.id)
    let result = currentArgs
    for (const other of groupPresets) {
        result = removePresetArgs(other, result)
    }
    return result
}

function applyPreset(preset: JavaArgumentPreset) {
    const argsToApply = getPresetArgs(preset)
    const cleanedRest = removeOtherGroupPresets(preset, split.value.rest)
    model.value = argsToApply + (cleanedRest ? ` ${cleanedRest}` : '')
}

function removePreset(preset: JavaArgumentPreset) {
    model.value = removePresetArgs(preset, model.value.trimStart()).trimStart()
}

function showPresets() {
    modal.value?.show()
}

async function copyPresetArgs(preset: JavaArgumentPreset) {
    const argsToCopy = getDisplayArgs(preset)
    await navigator.clipboard?.writeText(argsToCopy).catch(() => {})
    copiedPresetId.value = preset.id
    cancelCopiedReset?.()
    cancelCopiedReset = clock.after(1500, () => {
        if (copiedPresetId.value === preset.id) {
            copiedPresetId.value = null
        }
    })
}

function isPresetCollapsed(preset: JavaArgumentPreset) {
    return !expandedPresetIds.value.has(preset.id)
}

function togglePresetCollapsed(preset: JavaArgumentPreset) {
    const next = new Set(expandedPresetIds.value)
    if (next.has(preset.id)) {
        next.delete(preset.id)
    } else {
        next.add(preset.id)
    }
    expandedPresetIds.value = next
}

function isPresetActive(preset: JavaArgumentPreset) {
    return activePresets.value.some((p) => p.id === preset.id)
}

function isGroupCollapsed(group: string) {
    return !expandedGroupIds.value.has(group)
}

function toggleGroupCollapsed(group: string) {
    const next = new Set(expandedGroupIds.value)
    if (next.has(group)) {
        next.delete(group)
    } else {
        next.add(group)
    }
    expandedGroupIds.value = next
}

function getAutoResolvedLabel(preset: JavaArgumentPreset): string | null {
    if (preset.id !== 'gc-auto' || !preset.autoResolvedName) return null
    return formatMessage(messages.appJavaArgumentsPresetsGcAutoResolved, {
        strategy: preset.autoResolvedName,
    })
}

function getAutoReasonChainText(preset: JavaArgumentPreset): string | null {
    if (preset.id !== 'gc-auto' || !preset.autoReasonChain) return null
    const chain = preset.autoReasonChain
        .map((entry) => (typeof entry === 'string' ? entry : formatMessage(entry, entry.values)))
        .join(' → ')
    return formatMessage(messages.appJavaArgumentsPresetsGcAutoReasonChain, { chain })
}

function getAutoVerifiedLabel(preset: JavaArgumentPreset): string | null {
    if (preset.id !== 'gc-auto') return null
    const notice = lastGcLaunchReport.value
    if (!notice) return null
    let chosen = notice.chosen_strategy || 'JVM default GC'
    if (notice.pruned_args.length > 0) {
        chosen += ` (${notice.pruned_args.length} pruned)`
    }
    return formatMessage(messages.appJavaArgumentsPresetsGcAutoVerified, { chosen })
}

const tagsScrollRef = ref<HTMLElement | null>(null)
const showTagsFade = ref(false)
let tagsResizeObserver: ResizeObserver | null = null

function updateTagsFade() {
    const el = tagsScrollRef.value
    if (!el) return
    showTagsFade.value = el.scrollWidth > el.clientWidth + 1
}

watch(tagsScrollRef, (el) => {
    if (el) {
        updateTagsFade()
        tagsResizeObserver?.disconnect()
        tagsResizeObserver = new ResizeObserver(updateTagsFade)
        tagsResizeObserver.observe(el)
    } else {
        tagsResizeObserver?.disconnect()
        tagsResizeObserver = null
    }
})

watch(activePresets, () => {
    void nextTick(updateTagsFade)
})

onBeforeUnmount(() => {
    tagsResizeObserver?.disconnect()
    tagsResizeObserver = null
    cancelCopiedReset?.()
})
</script>

<template>
    <div class="flex flex-col gap-2">
        <div class="flex items-center gap-2">
            <div
                class="flex min-w-0 flex-1 items-center gap-2 rounded-xl bg-surface-4 px-3 transition-[box-shadow,color] focus-within:ring-4 focus-within:ring-brand-shadow"
                :class="props.disabled ? 'cursor-not-allowed opacity-50' : ''"
            >
                <div
                    v-if="activePresets.length"
                    ref="tagsScrollRef"
                    class="tags-scroll flex min-w-0 max-w-[50%] shrink-0 items-center gap-1 overflow-x-auto"
                    :class="{ 'tags-fade-right': showTagsFade }"
                    @scroll="updateTagsFade"
                >
                    <TagItem
                        v-for="preset in activePresets"
                        :key="preset.id"
                        class="shrink-0"
                        :action="props.disabled ? undefined : () => removePreset(preset)"
                        :aria-label="formatMessage(messages.appJavaArgumentsPresetsRemove)"
                    >
                        {{ formatMessage(preset.title) }}
                        <XIcon aria-hidden="true" />
                    </TagItem>
                </div>
                <input
                    :id="props.id"
                    :value="rest"
                    :disabled="props.disabled"
                    :placeholder="props.placeholder"
                    class="h-9 min-w-0 flex-1 bg-transparent px-0 py-2 text-base font-medium text-[var(--color-text-default)] placeholder:text-[var(--color-text-tertiary)] focus:text-[var(--color-text-primary)] focus:shadow-none focus:outline-none"
                    autocomplete="off"
                    type="text"
                    @input="onInput"
                />
            </div>
            <Button class="shrink-0" type="outlined" :disabled="props.disabled" @click="showPresets"
                ><SparklesIcon aria-hidden="true" />
                {{ formatMessage(messages.appJavaArgumentsPresetsButton) }}
            </Button>
        </div>

        <PreviewModal
            ref="modal"
            :header="formatMessage(messages.appJavaArgumentsPresetsModalTitle)"
            width="640px"
            max-width="640px"
        >
            <div class="flex flex-col gap-6">
                <div
                    v-for="groupEntry in groupedPresets"
                    :key="groupEntry.group"
                    class="flex flex-col gap-3"
                >
                    <div
                        role="button"
                        tabindex="0"
                        class="flex w-full cursor-pointer select-none items-center justify-between gap-2"
                        :aria-expanded="!isGroupCollapsed(groupEntry.group)"
                        :aria-label="
                            formatMessage(
                                isGroupCollapsed(groupEntry.group)
                                    ? messages.appJavaArgumentsPresetsExpandGroup
                                    : messages.appJavaArgumentsPresetsCollapseGroup,
                            )
                        "
                        @click="toggleGroupCollapsed(groupEntry.group)"
                        @keydown.enter="toggleGroupCollapsed(groupEntry.group)"
                        @keydown.space.prevent="toggleGroupCollapsed(groupEntry.group)"
                    >
                        <h3 class="m-0 text-lg font-semibold text-[var(--color-text-primary)]">
                            {{ formatMessage(groupEntry.title) }}
                        </h3>
                        <DropdownIcon
                            class="size-4 shrink-0 text-[var(--color-text-tertiary)] transition-transform"
                            :class="{ 'rotate-180': !isGroupCollapsed(groupEntry.group) }"
                            aria-hidden="true"
                        />
                    </div>

                    <Collapsible :collapsed="isGroupCollapsed(groupEntry.group)">
                        <div class="flex flex-col gap-3">
                            <div
                                v-for="preset in groupEntry.presets"
                                :key="preset.id"
                                class="flex flex-col gap-3 rounded-xl border border-solid border-surface-4 bg-surface-2 p-4"
                            >
                                <div class="flex items-start gap-3 text-left">
                                    <GlobeIcon
                                        class="mt-0.5 size-6 shrink-0 text-[var(--color-text-tertiary)]"
                                        aria-hidden="true"
                                    />
                                    <div class="min-w-0 flex-1">
                                        <p
                                            class="m-0 text-base font-semibold text-[var(--color-text-primary)]"
                                        >
                                            {{ formatMessage(preset.title) }}
                                        </p>
                                        <AutoLink
                                            :to="preset.link"
                                            target="_blank"
                                            rel="noreferrer"
                                            class="inline-flex items-start gap-1 text-sm text-[var(--color-text-tertiary)] hover:text-brand hover:underline"
                                        >
                                            <span class="min-w-0">{{
                                                formatMessage(preset.description)
                                            }}</span>
                                            <ExternalIcon
                                                class="mt-0.5 size-3.5 shrink-0"
                                                aria-hidden="true"
                                            />
                                        </AutoLink>
                                        <p
                                            v-if="showAutoDetails && getAutoResolvedLabel(preset)"
                                            class="m-0 mt-2 text-sm font-medium text-brand"
                                        >
                                            {{ getAutoResolvedLabel(preset) }}
                                        </p>
                                        <p
                                            v-if="showAutoDetails && getAutoReasonChainText(preset)"
                                            class="m-0 mt-1 text-xs text-[var(--color-text-tertiary)]"
                                        >
                                            {{ getAutoReasonChainText(preset) }}
                                        </p>
                                        <p
                                            v-if="showAutoDetails && getAutoVerifiedLabel(preset)"
                                            class="m-0 mt-1 text-xs text-warning-text"
                                        >
                                            {{ getAutoVerifiedLabel(preset) }}
                                        </p>
                                    </div>
                                    <Button
                                        :type="isPresetActive(preset) ? 'colored' : 'outlined'"
                                        color="brand"
                                        :disabled="isPresetActive(preset)"
                                        @click="applyPreset(preset)"
                                        ><CheckIcon
                                            v-if="isPresetActive(preset)"
                                            aria-hidden="true"
                                        />
                                        {{
                                            formatMessage(
                                                isPresetActive(preset)
                                                    ? messages.appJavaArgumentsPresetsApplied
                                                    : messages.appJavaArgumentsPresetsUse,
                                            )
                                        }}
                                    </Button>
                                </div>
                                <template v-if="preset.id !== 'gc-auto' || showAutoDetails">
                                    <div class="flex items-center gap-2">
                                        <div class="h-px min-w-0 flex-1 bg-surface-4" />
                                        <button
                                            v-tooltip="
                                                formatMessage(
                                                    messages.appJavaArgumentsPresetsArguments,
                                                )
                                            "
                                            type="button"
                                            :aria-label="
                                                formatMessage(
                                                    messages.appJavaArgumentsPresetsArguments,
                                                )
                                            "
                                            class="flex size-7 shrink-0 cursor-pointer items-center justify-center rounded-full border-none bg-transparent text-[var(--color-text-tertiary)] transition-colors hover:bg-surface-5 hover:text-[var(--color-text-primary)]"
                                            @click="togglePresetCollapsed(preset)"
                                        >
                                            <DropdownIcon
                                                class="size-4 transition-transform"
                                                :class="{
                                                    'rotate-180': !isPresetCollapsed(preset),
                                                }"
                                                aria-hidden="true"
                                            />
                                        </button>
                                    </div>
                                    <Collapsible :collapsed="isPresetCollapsed(preset)">
                                        <div class="flex items-start gap-2">
                                            <code
                                                class="min-w-0 flex-1 overflow-x-auto whitespace-pre-wrap break-all text-left font-mono text-xs leading-relaxed text-[var(--color-text-default)]"
                                            >
                                                {{ getDisplayArgs(preset) }}
                                            </code>
                                            <button
                                                type="button"
                                                :aria-label="
                                                    formatMessage(
                                                        messages.appJavaArgumentsPresetsArguments,
                                                    )
                                                "
                                                class="flex size-7 shrink-0 cursor-pointer items-center justify-center rounded-full border-none bg-transparent text-[var(--color-text-tertiary)] transition-colors hover:bg-surface-5 hover:text-[var(--color-text-primary)]"
                                                @click="copyPresetArgs(preset)"
                                            >
                                                <CheckIcon
                                                    v-if="copiedPresetId === preset.id"
                                                    class="size-4 text-green"
                                                    aria-hidden="true"
                                                />
                                                <CopyIcon
                                                    v-else
                                                    class="size-4"
                                                    aria-hidden="true"
                                                />
                                            </button>
                                        </div>
                                    </Collapsible>
                                </template>
                            </div>
                        </div>
                    </Collapsible>
                </div>
            </div>
        </PreviewModal>
    </div>
</template>

<style scoped>
.tags-scroll {
    scrollbar-width: none;
}

.tags-scroll::-webkit-scrollbar {
    display: none;
}

.tags-fade-right {
    mask-image: linear-gradient(to right, black 0%, black calc(100% - 1.25rem), transparent 100%);
    -webkit-mask-image: linear-gradient(
        to right,
        black 0%,
        black calc(100% - 1.25rem),
        transparent 100%
    );
}
</style>
