<script setup lang="ts">
import { Button, defineMessages, NewModal, useVIntl } from '@modrinth/ui'
import { computed, ref, watch } from 'vue'

import {
    applySchematicDownload,
    getSchematicDetail,
    type SchematicDetail,
    type SchematicItem,
} from '@/helpers/schematic-browser'

const props = defineProps<{
    item: SchematicItem | null
    /** Target instance for the download action; absent means browse-only. */
    instanceId?: string | null
}>()

const visible = defineModel<boolean>('visible', { required: true })

const emit = defineEmits<{
    (event: 'close'): void
}>()

const detail = ref<SchematicDetail | null>(null)
const loading = ref(false)

const downloading = ref(false)
const downloadedPath = ref<string | null>(null)
const downloadError = ref<string | null>(null)

const { formatMessage } = useVIntl()

const messages = defineMessages({
    title: {
        id: 'app.schematic.detail.title',
        defaultMessage: '{name}',
    },
    close: {
        id: 'app.schematic.detail.close',
        defaultMessage: 'Close',
    },
    copyLink: {
        id: 'app.schematic.detail.copy-link',
        defaultMessage: 'Copy source link',
    },
    openSourcePage: {
        id: 'app.schematic.detail.open-source-page',
        defaultMessage: 'Open source page',
    },
    description: {
        id: 'app.schematic.detail.description',
        defaultMessage: 'Description',
    },
    materials: {
        id: 'app.schematic.detail.materials',
        defaultMessage: 'Materials',
    },
    cached: {
        id: 'app.schematic.detail.cached',
        defaultMessage: 'cached',
    },
    format: {
        id: 'app.schematic.detail.format',
        defaultMessage: 'Format',
    },
    versions: {
        id: 'app.schematic.detail.versions',
        defaultMessage: 'Versions',
    },
    download: {
        id: 'app.schematic.detail.download',
        defaultMessage: 'Download',
    },
    downloading: {
        id: 'app.schematic.detail.downloading',
        defaultMessage: 'Downloading…',
    },
    downloaded: {
        id: 'app.schematic.detail.downloaded',
        defaultMessage: 'Saved to {path}',
    },
    downloadFailed: {
        id: 'app.schematic.detail.download-failed',
        defaultMessage: 'Download failed: {reason}',
    },
    noInstance: {
        id: 'app.schematic.detail.no-instance',
        defaultMessage: 'Choose an instance to download into.',
    },
    loadingDetail: {
        id: 'app.schematic.detail.loading',
        defaultMessage: 'Loading details…',
    },
    deliveryQuark: {
        id: 'app.schematic.detail.delivery.quark',
        defaultMessage:
            'This schematic is shared through a netdisk. Open the page to save it, then import the file into your instance.',
    },
    deliveryExternal: {
        id: 'app.schematic.detail.delivery.external',
        defaultMessage: 'No direct file is available. Open the source page to get it.',
    },
})

async function applyDownload() {
    const item = props.item
    if (!item || item.download.delivery !== 'direct') return
    if (!props.instanceId) {
        downloadError.value = formatMessage(messages.noInstance)
        return
    }
    downloading.value = true
    downloadError.value = null
    downloadedPath.value = null
    try {
        downloadedPath.value = await applySchematicDownload(
            props.instanceId,
            item.sourceId,
            item.id,
        )
    } catch (error) {
        downloadError.value = String(error)
    } finally {
        downloading.value = false
    }
}

function prevImage() {
    if (imageIndex.value > 0) imageIndex.value--
}

function nextImage() {
    if (images.value.length > 0 && imageIndex.value < images.value.length - 1) imageIndex.value++
}

function copyLink() {
    if (!props.item?.detailUrl) return
    navigator.clipboard.writeText(props.item.detailUrl).catch(() => {})
}

function openSourcePage() {
    if (!props.item?.detailUrl) return
    window.open(props.item.detailUrl, '_blank', 'noopener')
}

const deliveryHint = computed(() => {
    switch (props.item?.download.delivery) {
        case 'quark':
            return formatMessage(messages.deliveryQuark)
        case 'external':
            return formatMessage(messages.deliveryExternal)
        default:
            return null
    }
})

const canDownload = computed(() => props.item?.download.delivery === 'direct' && !!props.instanceId)

const imageIndex = ref(0)
const images = computed(() => detail.value?.images ?? props.item?.images ?? [])
const currentImage = computed(() => images.value[imageIndex.value] ?? null)

/**
 * The list payload carries enough to open the modal, but the description and
 * material list only come with the detail call. Reload whenever the modal opens
 * on a different item; a failed call keeps the list-level data on screen.
 */
watch(
    () => [props.item?.id, visible.value] as const,
    async ([, open]) => {
        if (!open || !props.item) return
        const current = props.item
        loading.value = true
        detail.value = null
        imageIndex.value = 0
        try {
            const result = await getSchematicDetail(current.sourceId, current.id)
            if (props.item?.id === current.id) detail.value = result
        } catch {
            // The list payload already renders; the detail fetch is additive.
        } finally {
            loading.value = false
        }
    },
    { immediate: true },
)

function close() {
    visible.value = false
    emit('close')
}

defineExpose({ close })
</script>

<template>
    <NewModal
        v-model="visible"
        :header="props.item ? formatMessage(messages.title, { name: props.item.title }) : ''"
        width="min(620px, calc(100vw - 2rem))"
        max-width="620px"
        scrollable
        max-content-height="min(42rem, 76vh)"
        @update:visible="(v) => !v && close()"
    >
        <!-- Images -->
        <div
            v-if="images.length"
            class="relative flex items-center justify-center rounded-md bg-surface-3 p-2"
        >
            <img
                :src="currentImage ?? ''"
                :alt="props.item?.title"
                class="max-h-64 object-contain"
            />
            <Button
                size="sm"
                type="quiet"
                class="absolute left-2 top-1/2 -translate-y-1/2"
                @click.stop="prevImage"
                >&lt;</Button
            >
            <Button
                size="sm"
                type="quiet"
                class="absolute right-2 top-1/2 -translate-y-1/2"
                @click.stop="nextImage"
                >&gt;</Button
            >
        </div>

        <!-- Description -->
        <section v-if="detail?.description" class="mt-2 text-sm">
            <h3 class="mb-1 font-semibold">{{ formatMessage(messages.description) }}</h3>
            <p class="break-words text-[var(--color-text-tertiary)]">{{ detail.description }}</p>
        </section>

        <!-- Format & Versions -->
        <section
            v-if="props.item"
            class="grid grid-cols-2 gap-x-4 gap-y-2 text-xs text-[var(--color-text-tertiary)]"
        >
            <dl>
                <dt>{{ formatMessage(messages.format) }}</dt>
                <dd>{{ props.item.format.toUpperCase() }}</dd>
            </dl>
            <dl>
                <dt>{{ formatMessage(messages.versions) }}</dt>
                <dd v-if="props.item.versions?.length">{{ props.item.versions.join(', ') }}</dd>
                <dd v-else>-</dd>
            </dl>
        </section>

        <!-- Materials -->
        <section v-if="detail?.materials?.length" class="mt-2">
            <h3 class="mb-2 font-semibold">{{ formatMessage(messages.materials) }}</h3>
            <ul class="list-disc pl-5 text-xs leading-relaxed">
                <li
                    v-for="material in detail.materials"
                    :key="material.name"
                    class="truncate"
                    :title="material.name"
                >
                    {{ material.name }} × {{ material.count }}
                </li>
            </ul>
        </section>

        <!-- Delivery hint -->
        <div v-if="deliveryHint" class="mt-2 rounded border border-orange bg-orange/10 p-2 text-xs">
            {{ deliveryHint }}
        </div>

        <!-- Download result -->
        <div v-if="downloadedPath" class="mt-2 break-all rounded bg-green/10 p-2 text-xs">
            {{ formatMessage(messages.downloaded, { path: downloadedPath }) }}
        </div>
        <div v-if="downloadError" class="mt-2 rounded bg-red/10 p-2 text-xs">
            {{ formatMessage(messages.downloadFailed, { reason: downloadError }) }}
        </div>

        <!-- Actions -->
        <footer class="mt-2 flex flex-wrap items-center justify-end gap-2">
            <Button
                v-if="props.item?.download.delivery === 'direct'"
                size="sm"
                type="colored"
                :disabled="downloading || !canDownload"
                :title="!props.instanceId ? formatMessage(messages.noInstance) : undefined"
                @click="applyDownload"
            >
                {{
                    downloading
                        ? formatMessage(messages.downloading)
                        : formatMessage(messages.download)
                }}
            </Button>
            <Button size="sm" type="quiet" @click="copyLink">{{
                formatMessage(messages.copyLink)
            }}</Button>
            <Button size="sm" type="outlined" @click="openSourcePage">{{
                formatMessage(messages.openSourcePage)
            }}</Button>
            <Button size="sm" type="quiet" @click="close">{{
                formatMessage(messages.close)
            }}</Button>
        </footer>
    </NewModal>
</template>
