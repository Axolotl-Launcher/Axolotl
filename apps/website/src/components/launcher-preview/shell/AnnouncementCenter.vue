<script setup lang="ts">
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { commonMessages } from '@modrinth/ui/src/utils/common-messages.ts'
import { computed, nextTick, reactive, ref } from 'vue'

import announcementSnapshot from '../data/announcements.json'
import { announcementMessages as messages } from '../lib/messages'
import PreviewModal from '../overlays/PreviewModal.vue'

/**
 * The announcement center of the app's RemoteAnnouncements: a list of the
 * published announcements beside the selected one. Its items are the real
 * public announcements endpoint's response captured in `data/announcements.json`.
 */
interface Announcement {
    id: string
    title: string
    summary: string | null
    content: string
    published_at: string
}

const { formatMessage } = useVIntl()

const items = computed(() => announcementSnapshot.announcements as Announcement[])
const read = reactive(new Set<string>())
const selected = ref<Announcement | null>(null)
const modal = ref<InstanceType<typeof PreviewModal>>()

const unreadCount = computed(() => items.value.filter((item) => !read.has(item.id)).length)

function selectAnnouncement(item: Announcement) {
    selected.value = item
    read.add(item.id)
}

function markAllRead() {
    for (const item of items.value) read.add(item.id)
}

function open() {
    if (!selected.value && items.value.length) selectAnnouncement(items.value[0])
    void nextTick(() => modal.value?.show())
}

defineExpose({ open, unreadCount })
</script>

<template>
    <PreviewModal
        ref="modal"
        :header="formatMessage(messages.centerTitle)"
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
                        formatMessage(messages.centerTitle)
                    }}</span>
                    <Button v-if="unreadCount > 0" @click="markAllRead">{{
                        formatMessage(messages.readAll)
                    }}</Button>
                </div>
                <div
                    v-if="!items.length"
                    class="py-8 text-center text-sm text-[var(--color-text-tertiary)]"
                >
                    {{ formatMessage(messages.empty) }}
                </div>
                <div v-else class="flex min-h-0 flex-col gap-1 overflow-auto">
                    <button
                        v-for="item in items"
                        :key="item.id"
                        class="flex items-start gap-2 rounded-lg p-2 text-left hover:bg-surface-4"
                        :class="{ 'bg-surface-4': selected?.id === item.id }"
                        @click="selectAnnouncement(item)"
                    >
                        <span
                            class="mt-1.5 size-2 shrink-0 rounded-full"
                            :class="read.has(item.id) ? 'opacity-0' : 'bg-red'"
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
                <p class="m-0 whitespace-pre-wrap break-words">{{ selected.content }}</p>
            </section>
            <div
                v-else
                class="flex flex-1 items-center justify-center text-sm text-[var(--color-text-tertiary)]"
            >
                {{ formatMessage(messages.empty) }}
            </div>
        </div>
        <template #actions>
            <div class="flex flex-wrap justify-end gap-2">
                <Button @click="modal?.hide()">{{
                    formatMessage(commonMessages.closeButton)
                }}</Button>
            </div>
        </template>
    </PreviewModal>
</template>
