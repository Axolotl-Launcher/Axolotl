<template>
    <span
        ref="trigger"
        class="relative inline-flex max-w-full"
        @mouseenter="show"
        @mouseleave="scheduleClose"
        @focusin="show"
        @focusout="scheduleClose"
    >
        <button
            type="button"
            class="cursor-pointer border-0 bg-transparent p-0 text-inherit underline decoration-dotted underline-offset-4"
            @click="show"
        >
            {{ label }}
        </button>
        <Teleport :to="layer" :disabled="!layer">
            <div
                v-if="open"
                ref="popup"
                class="fixed z-[200] flex w-96 flex-col gap-2 rounded-lg border border-solid border-surface-5 bg-surface-3 p-3 text-left shadow-xl"
                :style="position"
                @mouseenter="cancelClose"
                @mouseleave="scheduleClose"
                @focusin="cancelClose"
                @focusout="scheduleClose"
            >
                <strong>{{ label }}</strong>
                <p v-if="loading" class="m-0 text-sm">{{ formatMessage(messages.loading) }}</p>
                <MarkdownBody
                    v-else-if="changelog"
                    class="max-h-64 min-h-0 overflow-y-auto text-sm"
                    :source="changelog"
                />
                <p v-else class="m-0 text-sm">{{ formatMessage(messages.empty) }}</p>
            </div>
        </Teleport>
    </span>
</template>
<script setup lang="ts">
import './messages'

import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'

import { injectPreviewWindow } from '../../../lib/window'
import MarkdownBody from '../../discover/components/project/MarkdownBody.vue'
import { loadUpgradeVersionMetadata } from './metadata'
const props = defineProps<{
    label: string
    provider: string | null
    projectId: string | null
    releaseId: string | null
}>()
const { formatMessage } = useVIntl()
const messages = defineMessages({
    loading: {
        id: 'instance.upgrade.changelog.loading',
        defaultMessage: 'Loading release details…',
    },
    empty: {
        id: 'instance.upgrade.changelog.empty',
        defaultMessage: 'No changelog was provided for this version.',
    },
})
const { layer, root, toLocal, scale } = injectPreviewWindow()
const trigger = ref<HTMLElement | null>(null)
const popup = ref<HTMLElement | null>(null)
const position = ref<Record<string, string>>({})
const open = ref(false)
const loading = ref(false)
const changelog = ref<string | null>(null)
let loaded = false
let generation = 0
let closeTimer: ReturnType<typeof setTimeout> | undefined
function cancelClose() {
    clearTimeout(closeTimer)
}
function scheduleClose() {
    cancelClose()
    closeTimer = setTimeout(() => {
        open.value = false
    }, 160)
}
function reposition() {
    if (!open.value || !trigger.value || !popup.value || !root.value) return
    const bounds = trigger.value.getBoundingClientRect()
    const windowBounds = root.value.getBoundingClientRect()
    if (bounds.bottom < windowBounds.top || bounds.top > windowBounds.bottom) {
        open.value = false
        return
    }
    const anchor = toLocal(bounds.left, bounds.bottom)
    const width = windowBounds.width / scale.value
    const height = windowBounds.height / scale.value
    const above = anchor.y + popup.value.offsetHeight + 8 > height
    position.value = {
        left: `${Math.max(8, Math.min(anchor.x, width - popup.value.offsetWidth - 8))}px`,
        top: `${Math.max(8, above ? anchor.y - bounds.height / scale.value - popup.value.offsetHeight - 8 : anchor.y + 8)}px`,
    }
}
async function show() {
    cancelClose()
    open.value = true
    await nextTick()
    reposition()
    if (loaded || loading.value || !props.provider || !props.projectId || !props.releaseId) return
    const request = ++generation
    loaded = true
    loading.value = true
    try {
        const metadata = await loadUpgradeVersionMetadata(
            props.provider,
            props.projectId,
            props.releaseId,
        )
        if (request === generation) changelog.value = metadata?.changelog ?? null
    } finally {
        if (request === generation) loading.value = false
        await nextTick()
        reposition()
    }
}
function outside(event: PointerEvent) {
    if (
        !(event.target instanceof Node) ||
        trigger.value?.contains(event.target) ||
        popup.value?.contains(event.target)
    )
        return
    open.value = false
}
function escape(event: KeyboardEvent) {
    if (event.key === 'Escape') open.value = false
}
watch(
    () => [props.provider, props.projectId, props.releaseId],
    () => {
        generation++
        loaded = false
        loading.value = false
        changelog.value = null
        open.value = false
    },
)
onMounted(() => {
    window.addEventListener('resize', reposition)
    window.addEventListener('scroll', reposition, true)
    document.addEventListener('pointerdown', outside)
    document.addEventListener('keydown', escape)
})
onBeforeUnmount(() => {
    generation++
    cancelClose()
    window.removeEventListener('resize', reposition)
    window.removeEventListener('scroll', reposition, true)
    document.removeEventListener('pointerdown', outside)
    document.removeEventListener('keydown', escape)
})
</script>
