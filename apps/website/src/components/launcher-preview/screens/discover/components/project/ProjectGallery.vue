<script setup lang="ts">
import CalendarIcon from '@modrinth/assets/icons/calendar.svg?component'
import ContractIcon from '@modrinth/assets/icons/contract.svg?component'
import ExpandIcon from '@modrinth/assets/icons/expand.svg?component'
import ExternalIcon from '@modrinth/assets/icons/external.svg?component'
import LeftArrowIcon from '@modrinth/assets/icons/left-arrow.svg?component'
import RightArrowIcon from '@modrinth/assets/icons/right-arrow.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import ButtonLink from '@modrinth/ui/src/components/base/buttons/ButtonLink.vue'
import { useFormatDateTime } from '@modrinth/ui/src/composables/format-date-time.ts'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import {
    commonMessages,
    commonProjectSettingsMessages,
} from '@modrinth/ui/src/utils/common-messages.ts'
import { computed, onMounted, onUnmounted, ref } from 'vue'

import type { ProjectDetail } from '../../../../data'
import PreviewModal from '../../../../overlays/PreviewModal.vue'
import { messages } from '../../messages'

/**
 * The Gallery tab (apps/app-frontend/src/pages/project/Gallery.vue): image cards and the
 * viewer modal with previous/next, zoom and arrow-key navigation. Images are the snapshot's
 * local copies.
 */
const props = defineProps<{ project: ProjectDetail }>()

type GalleryImage = ProjectDetail['gallery'][number]
interface GalleryEntry {
    image: GalleryImage
    index: number
}

const MC_SERVER_BANNER_NAME = '__mc_server_banner__'

const { formatMessage } = useVIntl()
const formatDate = useFormatDateTime({ year: 'numeric', month: 'long', day: 'numeric' })

const filteredGallery = computed<GalleryEntry[]>(() =>
    (props.project.gallery ?? [])
        .map((image, index) => ({ image, index }))
        .filter(({ image }) => image.title !== MC_SERVER_BANNER_NAME),
)
const selectedGalleryItem = ref<GalleryEntry | null>(null)
const zoomedIn = ref(false)
const viewerModal = ref<InstanceType<typeof PreviewModal>>()

function galleryText(entry: GalleryEntry, field: 'title' | 'description') {
    return entry.image[field] ?? ''
}

const viewerTitle = computed(() => {
    if (!selectedGalleryItem.value) return formatMessage(commonProjectSettingsMessages.gallery)
    return (
        galleryText(selectedGalleryItem.value, 'title') ||
        formatMessage(commonProjectSettingsMessages.gallery)
    )
})

function imageAlt(entry: GalleryEntry) {
    return galleryText(entry, 'title') || formatMessage(commonProjectSettingsMessages.gallery)
}

function formatCreated(entry: GalleryEntry) {
    return entry.image.created ? formatDate(new Date(entry.image.created)) : ''
}

function viewImage(entry: GalleryEntry) {
    selectedGalleryItem.value = entry
    zoomedIn.value = false
    viewerModal.value?.show()
}

function changeImage(offset: number) {
    if (!selectedGalleryItem.value || filteredGallery.value.length < 2) return
    const currentIndex = filteredGallery.value.findIndex(
        (entry) => entry.index === selectedGalleryItem.value?.index,
    )
    const nextIndex =
        (currentIndex + offset + filteredGallery.value.length) % filteredGallery.value.length
    selectedGalleryItem.value = filteredGallery.value[nextIndex]
    zoomedIn.value = false
}

function handleViewerHide() {
    selectedGalleryItem.value = null
    zoomedIn.value = false
}

function keyListener(event: KeyboardEvent) {
    if (!selectedGalleryItem.value) return
    if (event.key === 'ArrowLeft') {
        event.preventDefault()
        changeImage(-1)
    } else if (event.key === 'ArrowRight') {
        event.preventDefault()
        changeImage(1)
    }
}

onMounted(() => window.addEventListener('keydown', keyListener))
onUnmounted(() => window.removeEventListener('keydown', keyListener))
</script>

<template>
    <div class="grid grid-cols-[repeat(auto-fill,minmax(17rem,1fr))] gap-4">
        <article
            v-for="entry in filteredGallery"
            :key="entry.image.url"
            class="group overflow-hidden rounded-2xl border border-solid border-surface-5 bg-surface-2 transition-colors hover:border-brand"
        >
            <button
                class="relative block aspect-video w-full cursor-zoom-in overflow-hidden border-0 bg-surface-1 p-0"
                :aria-label="formatMessage(commonMessages.viewLabel)"
                @click="viewImage(entry)"
            >
                <img
                    :src="entry.image.url"
                    :alt="imageAlt(entry)"
                    loading="lazy"
                    decoding="async"
                    class="size-full object-cover transition-transform duration-200 group-hover:scale-[1.02]"
                />
            </button>
            <div class="flex min-h-0 flex-1 flex-col gap-2 p-3">
                <div
                    v-if="galleryText(entry, 'title') || galleryText(entry, 'description')"
                    class="min-w-0"
                >
                    <h3
                        v-if="galleryText(entry, 'title')"
                        class="m-0 break-words font-semibold text-[var(--color-text-primary)]"
                    >
                        {{ galleryText(entry, 'title') }}
                    </h3>
                    <p
                        v-if="galleryText(entry, 'description')"
                        class="mb-0 mt-1 break-words text-[var(--color-text-tertiary)]"
                    >
                        {{ galleryText(entry, 'description') }}
                    </p>
                </div>
                <div
                    class="mt-auto flex items-center gap-2 text-sm text-[var(--color-text-tertiary)]"
                >
                    <CalendarIcon class="size-4 shrink-0" aria-hidden="true" />
                    {{ formatCreated(entry) }}
                </div>
            </div>
        </article>
    </div>

    <PreviewModal
        ref="viewerModal"
        max-width="92rem"
        width="calc(1280px - 4rem)"
        :no-padding="true"
        :header="viewerTitle"
        :on-hide="handleViewerHide"
    >
        <div
            v-if="selectedGalleryItem"
            class="relative flex max-h-[calc(800px-13rem)] min-h-64 w-full items-center justify-center overflow-hidden bg-surface-1"
        >
            <div
                class="flex max-h-[calc(800px-13rem)] w-full min-w-0 items-center justify-center overflow-auto p-4"
            >
                <img
                    :src="selectedGalleryItem.image.url"
                    :alt="imageAlt(selectedGalleryItem)"
                    :class="
                        zoomedIn
                            ? 'max-w-none cursor-zoom-out'
                            : 'max-h-[calc(800px-15rem)] max-w-full cursor-zoom-in'
                    "
                    @click="zoomedIn = !zoomedIn"
                />
            </div>
            <Button
                v-if="filteredGallery.length > 1"
                circular
                icon-only
                class="absolute left-4 top-1/2 -translate-y-1/2"
                :aria-label="formatMessage(commonMessages.backButton)"
                @click="changeImage(-1)"
                ><LeftArrowIcon />
            </Button>
            <Button
                v-if="filteredGallery.length > 1"
                circular
                icon-only
                class="absolute right-4 top-1/2 -translate-y-1/2"
                :aria-label="formatMessage(commonMessages.nextButton)"
                @click="changeImage(1)"
                ><RightArrowIcon />
            </Button>
        </div>
        <template #actions>
            <div
                v-if="selectedGalleryItem"
                class="flex flex-wrap items-center justify-between gap-2"
            >
                <div class="min-w-0 text-sm text-[var(--color-text-tertiary)]">
                    <p
                        v-if="galleryText(selectedGalleryItem, 'description')"
                        class="m-0 break-words"
                    >
                        {{ galleryText(selectedGalleryItem, 'description') }}
                    </p>
                    <div class="mt-1 flex items-center gap-2">
                        <CalendarIcon class="size-4 shrink-0" aria-hidden="true" />
                        {{ formatCreated(selectedGalleryItem) }}
                    </div>
                </div>
                <div class="flex flex-wrap items-center gap-2">
                    <Button @click="zoomedIn = !zoomedIn"
                        ><ContractIcon v-if="zoomedIn" />
                        <ExpandIcon v-else />
                        {{ formatMessage(zoomedIn ? messages.zoomOut : messages.zoomIn) }}
                    </Button>
                    <ButtonLink
                        target="_blank"
                        rel="noreferrer"
                        :href="selectedGalleryItem.image.url"
                    >
                        <ExternalIcon />
                        {{ formatMessage(commonMessages.openInBrowserButton) }}
                    </ButtonLink>
                </div>
            </div>
        </template>
    </PreviewModal>
</template>
