<script setup lang="ts">
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { useModalStack } from '@modrinth/ui/src/composables/modal-stack.ts'
import { commonMessages } from '@modrinth/ui/src/utils/common-messages.ts'
import {
    computed,
    defineAsyncComponent,
    nextTick,
    onUnmounted,
    reactive,
    ref,
    shallowRef,
    watch,
} from 'vue'

import { usePreview } from '../../../../lib/state/index'
import { injectPreviewWindow } from '../../../../lib/window'
import PreviewModal from '../../../../overlays/PreviewModal.vue'
import { messages } from '../../messages'

/**
 * The preview-only instance of the app's RemoteAnnouncements
 * (apps/app-frontend/src/components/ui/RemoteAnnouncements.vue) behind the
 * developer "Announcement preview" buttons: it never fetches or persists, and
 * shows local sample content through the real startup modal and popup paths.
 */
type RemoteAnnouncement = {
    id: string
    title: string
    summary: string | null
    content: string
    type: 'modal' | 'notification'
    priority: 'low' | 'normal' | 'high' | 'critical'
    starts_at: string
    ends_at: string | null
    published_at: string
    action_url: string | null
    action_label: string | null
}

const props = defineProps<{ ready: boolean }>()

// The project page's Markdown renderer (the app's `renderString` port).
const MarkdownBody = defineAsyncComponent(
    () => import('../../../discover/components/project/MarkdownBody.vue'),
)

const { formatMessage } = useVIntl()
const { hasModal } = useModalStack()
const { clock } = usePreview()
const { layer } = injectPreviewWindow()

const modal = ref<InstanceType<typeof PreviewModal>>()
const selected = ref<RemoteAnnouncement | null>(null)
const active = ref(false)
const centerOpen = ref(false)
const startupNotice = ref<RemoteAnnouncement | null>(null)
const read = reactive(new Set<string>())
const queuedThisSession = new Set<string>()
const startupNotified = new Set<string>()
const items = shallowRef<RemoteAnnouncement[]>([])
let pending: RemoteAnnouncement[] = []
let disposed = false
let cancelAdvance: (() => void) | undefined
let cancelNotice: (() => void) | undefined

function announcementKey(item: RemoteAnnouncement) {
    return item.id + ':' + item.published_at
}

const hasUnread = computed(() => items.value.some((item) => !read.has(announcementKey(item))))

function isAnnouncementActive(item: RemoteAnnouncement, now = Date.now()) {
    return Date.parse(item.starts_at) <= now && (!item.ends_at || Date.parse(item.ends_at) > now)
}

function safeAnnouncementUrl(value: unknown): string | null {
    if (typeof value !== 'string') return null
    try {
        const url = new URL(value)
        return ['https:', 'http:'].includes(url.protocol) && !url.username && !url.password
            ? url.href
            : null
    } catch {
        return null
    }
}

function scheduleAdvance() {
    cancelAdvance?.()
    cancelAdvance = clock.after(350, advance)
}

function openCenter(item?: RemoteAnnouncement) {
    if (!props.ready || disposed) return
    centerOpen.value = true
    if (item) {
        startupNotice.value = null
        selectAnnouncement(item)
    } else if (!selected.value && items.value.length) {
        selectAnnouncement(items.value[0])
    }
    void nextTick(() => modal.value?.show())
}

async function show(item: RemoteAnnouncement) {
    if (
        disposed ||
        !props.ready ||
        !isAnnouncementActive(item) ||
        (hasModal.value && !active.value && !centerOpen.value)
    )
        return
    centerOpen.value = true
    active.value = true
    selectAnnouncement(item)
    await nextTick()
    if (!disposed) modal.value?.show()
}

function selectAnnouncement(item: RemoteAnnouncement) {
    selected.value = item
    const key = announcementKey(item)
    read.add(key)
    pending = pending.filter((entry) => announcementKey(entry) !== key)
}

function markAllRead() {
    for (const item of items.value) read.add(announcementKey(item))
    pending = []
}

function advance() {
    if (disposed || !props.ready || hasModal.value || active.value) return
    const next = pending.shift()
    if (next) {
        void show(next)
        return
    }
    for (const item of items.value) {
        const key = announcementKey(item)
        if (
            item.type === 'notification' &&
            !startupNotified.has(key) &&
            !read.has(key) &&
            isAnnouncementActive(item)
        ) {
            startupNotice.value = item
            startupNotified.add(key)
            cancelNotice?.()
            cancelNotice = clock.after(8000, () => {
                if (startupNotice.value === item) {
                    startupNotice.value = null
                    advance()
                }
            })
            break
        }
    }
}

function closed() {
    active.value = false
    centerOpen.value = false
    scheduleAdvance()
}

function sync(next: RemoteAnnouncement[], fresh: boolean) {
    items.value = next.filter((item) => isAnnouncementActive(item))
    const keys = new Set(items.value.map(announcementKey))
    pending = pending.filter((item) => keys.has(announcementKey(item)))
    if (selected.value && !keys.has(announcementKey(selected.value))) selected.value = null
    if (fresh) {
        for (const item of items.value) {
            const key = announcementKey(item)
            if (
                item.type === 'modal' &&
                !queuedThisSession.has(key) &&
                (!read.has(key) || item.priority === 'critical')
            ) {
                queuedThisSession.add(key)
                pending.push(item)
            }
        }
        advance()
    }
}

/** `openUrl`: the system browser, a new tab here. */
function openLink(value: unknown) {
    const url = safeAnnouncementUrl(value)
    if (!url) return
    window.open(url, '_blank', 'noopener')
}

function preview(type: RemoteAnnouncement['type'], withAction = false) {
    if (!props.ready || hasModal.value || disposed) return
    const now = new Date().toISOString()
    const item: RemoteAnnouncement = {
        id: 'local-preview-' + type,
        title: formatMessage(messages.appRemoteAnnouncementsPreviewTitle),
        summary: formatMessage(messages.appRemoteAnnouncementsPreviewSummary),
        content: formatMessage(messages.appRemoteAnnouncementsPreviewContent),
        type,
        priority: 'normal',
        starts_at: now,
        ends_at: null,
        published_at: now,
        action_label: withAction
            ? formatMessage(messages.appRemoteAnnouncementsPreviewAction)
            : null,
        action_url: withAction ? 'https://axlmc.org' : null,
    }
    read.clear()
    queuedThisSession.clear()
    sync([item], true)
}

defineExpose({ preview })

watch([() => props.ready, hasModal], scheduleAdvance)

onUnmounted(() => {
    disposed = true
    cancelAdvance?.()
    cancelNotice?.()
})
</script>

<template>
    <Teleport :to="layer" :disabled="!layer">
        <div
            v-if="startupNotice"
            class="fixed right-4 top-16 z-50 w-[22rem] max-w-[calc(1280px-2rem)] rounded-xl border border-surface-5 bg-surface-3 p-3 shadow-lg"
        >
            <button class="w-full text-left" @click="startupNotice && openCenter(startupNotice)">
                <div class="mb-1 text-xs font-semibold uppercase text-brand">
                    {{ formatMessage(messages.appRemoteAnnouncementsUnread) }}
                </div>
                <div class="font-semibold text-[var(--color-text-primary)]">
                    {{ startupNotice.title }}
                </div>
                <div
                    v-if="startupNotice.summary"
                    class="mt-1 line-clamp-2 text-sm text-[var(--color-text-tertiary)]"
                >
                    {{ startupNotice.summary }}
                </div>
            </button>
        </div>
    </Teleport>
    <PreviewModal
        ref="modal"
        :header="formatMessage(messages.appRemoteAnnouncementsCenterTitle)"
        :on-hide="closed"
        max-width="1024px"
        width="1024px"
        max-content-height="512px"
        scrollable
    >
        <div class="flex h-[512px] min-h-0 gap-3">
            <aside
                class="flex min-h-0 w-[32%] min-w-[14rem] flex-col border-r border-surface-5 pr-3"
            >
                <div class="mb-2 flex items-center justify-between">
                    <span class="font-semibold text-[var(--color-text-primary)]">{{
                        formatMessage(messages.appRemoteAnnouncementsCenterTitle)
                    }}</span>
                    <Button v-if="hasUnread" @click="markAllRead">{{
                        formatMessage(messages.appRemoteAnnouncementsReadAll)
                    }}</Button>
                </div>
                <div
                    v-if="!items.length"
                    class="py-8 text-center text-sm text-[var(--color-text-tertiary)]"
                >
                    {{ formatMessage(messages.appRemoteAnnouncementsEmpty) }}
                </div>
                <div v-else class="flex min-h-0 flex-col gap-1 overflow-auto">
                    <button
                        v-for="item in items"
                        :key="announcementKey(item)"
                        class="flex items-start gap-2 rounded-lg p-2 text-left hover:bg-surface-4"
                        :class="{
                            'bg-surface-4':
                                selected && announcementKey(selected) === announcementKey(item),
                        }"
                        @click="selectAnnouncement(item)"
                    >
                        <span
                            class="mt-1.5 size-2 shrink-0 rounded-full"
                            :class="read.has(announcementKey(item)) ? 'opacity-0' : 'bg-red'"
                        />
                        <span class="min-w-0 flex-1">
                            <span
                                class="block truncate font-medium text-[var(--color-text-primary)]"
                                >{{ item.title }}</span
                            >
                            <span
                                class="line-clamp-2 block text-xs text-[var(--color-text-tertiary)]"
                                >{{ item.summary || item.content }}</span
                            >
                        </span>
                    </button>
                </div>
            </aside>
            <section v-if="selected" class="min-h-0 min-w-0 flex-1 overflow-auto px-1">
                <h2 class="mb-4 text-xl font-semibold text-[var(--color-text-primary)]">
                    {{ selected.title }}
                </h2>
                <MarkdownBody :source="selected.content" class="break-words" />
                <div class="mt-4 flex justify-end">
                    <Button
                        v-if="selected.action_url && selected.action_label"
                        type="colored"
                        color="brand"
                        @click="openLink(selected.action_url)"
                        >{{ selected.action_label }}</Button
                    >
                </div>
            </section>
            <div
                v-else
                class="flex flex-1 items-center justify-center text-sm text-[var(--color-text-tertiary)]"
            >
                {{ formatMessage(messages.appRemoteAnnouncementsEmpty) }}
            </div>
        </div>
        <template #actions>
            <div class="flex flex-wrap justify-end gap-2">
                <Button @click="modal?.hide()"
                    >{{ formatMessage(commonMessages.closeButton) }}
                </Button>
            </div>
        </template>
    </PreviewModal>
</template>
