<script setup lang="ts">
import {
    CalendarIcon,
    ContractIcon,
    ExpandIcon,
    ExternalIcon,
    LeftArrowIcon,
    RightArrowIcon,
} from '@modrinth/assets'
import {
    Button,
    ButtonLink,
    commonMessages,
    commonProjectSettingsMessages,
    defineMessages,
    NewModal,
    useFormatDateTime,
    useVIntl,
} from '@modrinth/ui'
import { computed, onMounted, onUnmounted, ref } from 'vue'

import { trackEvent } from '@/helpers/analytics'
import {
    type ProjectGalleryCaptionField,
    projectGalleryTranslationSegmentId,
    visibleProjectGallery,
} from '@/helpers/project-gallery'
import type { TranslationMode, TranslationStyle } from '@/helpers/translation'

interface GalleryImage {
    url: string
    raw_url?: string
    title?: string
    description?: string
    created: string
}

interface GalleryEntry {
    image: GalleryImage
    index: number
}

const props = withDefaults(
    defineProps<{
        project: {
            id: string
            gallery?: GalleryImage[]
        }
        translationActive?: boolean
        translations?: Record<string, string>
        translationMode?: TranslationMode
        translationStyle?: TranslationStyle
    }>(),
    {
        translationActive: false,
        translations: () => ({}),
        translationMode: 'bilingual',
        translationStyle: 'weakened',
    },
)

const { formatMessage } = useVIntl()
const formatDate = useFormatDateTime({
    year: 'numeric',
    month: 'long',
    day: 'numeric',
})

const screenshotMessages = defineMessages({
    zoomIn: {
        id: 'app.instance.screenshots.zoom-in',
        defaultMessage: 'View at full size',
    },
    zoomOut: {
        id: 'app.instance.screenshots.zoom-out',
        defaultMessage: 'Fit to window',
    },
})

const filteredGallery = computed<GalleryEntry[]>(() => visibleProjectGallery(props.project.gallery))
const selectedGalleryItem = ref<GalleryEntry | null>(null)
const zoomedIn = ref(false)
const viewerModal = ref<InstanceType<typeof NewModal>>()

const viewerTitle = computed(() => {
    if (!selectedGalleryItem.value) return formatMessage(commonProjectSettingsMessages.gallery)
    return (
        galleryText(selectedGalleryItem.value, 'title') ||
        formatMessage(commonProjectSettingsMessages.gallery)
    )
})

const viewerImageUrl = computed(() => {
    if (!selectedGalleryItem.value) return ''
    return zoomedIn.value
        ? (selectedGalleryItem.value.image.raw_url ?? selectedGalleryItem.value.image.url)
        : selectedGalleryItem.value.image.url
})

const translationClass = computed(() => [
    'gallery-translation',
    `gallery-translation--${props.translationStyle}`,
])

function translationFor(
    entry: GalleryEntry,
    field: ProjectGalleryCaptionField,
): string | undefined {
    if (!props.translationActive) return undefined
    return (
        props.translations[projectGalleryTranslationSegmentId(entry.index, field)]?.trim() ||
        undefined
    )
}

function galleryText(entry: GalleryEntry, field: ProjectGalleryCaptionField): string {
    const original = entry.image[field] ?? ''
    const translated = translationFor(entry, field)
    return props.translationMode === 'translation-only' && translated ? translated : original
}

function showBilingualTranslation(entry: GalleryEntry, field: ProjectGalleryCaptionField): boolean {
    return props.translationMode === 'bilingual' && !!translationFor(entry, field)
}

function imageAlt(entry: GalleryEntry): string {
    return galleryText(entry, 'title') || formatMessage(commonProjectSettingsMessages.gallery)
}

function viewImage(entry: GalleryEntry) {
    selectedGalleryItem.value = entry
    zoomedIn.value = false
    viewerModal.value?.show()

    trackEvent('GalleryImageExpand', {
        project_id: props.project.id,
        url: entry.image.url,
    })
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

    trackEvent(offset > 0 ? 'GalleryImageNext' : 'GalleryImagePrevious', {
        project_id: props.project.id,
        url: selectedGalleryItem.value.image.url,
    })
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

onMounted(() => {
    window.addEventListener('keydown', keyListener)
})

onUnmounted(() => {
    window.removeEventListener('keydown', keyListener)
})
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
                    referrerpolicy="no-referrer"
                    loading="lazy"
                    decoding="async"
                    :src="entry.image.url"
                    :alt="imageAlt(entry)"
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
                    <p v-if="showBilingualTranslation(entry, 'title')" :class="translationClass">
                        {{ translationFor(entry, 'title') }}
                    </p>
                    <p
                        v-if="galleryText(entry, 'description')"
                        class="mb-0 mt-1 break-words text-[var(--color-text-tertiary)]"
                    >
                        {{ galleryText(entry, 'description') }}
                    </p>
                    <p
                        v-if="showBilingualTranslation(entry, 'description')"
                        :class="translationClass"
                    >
                        {{ translationFor(entry, 'description') }}
                    </p>
                </div>
                <div
                    class="mt-auto flex items-center gap-2 text-sm text-[var(--color-text-tertiary)]"
                >
                    <CalendarIcon class="size-4 shrink-0" aria-hidden="true" />
                    {{ formatDate(new Date(entry.image.created)) }}
                </div>
            </div>
        </article>
    </div>

    <NewModal
        ref="viewerModal"
        :max-width="'92rem'"
        :width="'calc(100vw - 4rem)'"
        :no-padding="true"
        :header="viewerTitle"
        :on-hide="handleViewerHide"
    >
        <div
            v-if="selectedGalleryItem"
            class="relative flex w-full min-h-64 max-h-[calc(100vh-13rem)] items-center justify-center overflow-hidden bg-surface-1"
        >
            <div
                class="min-w-0 flex max-h-[calc(100vh-13rem)] w-full items-center justify-center overflow-auto p-4"
            >
                <img
                    referrerpolicy="no-referrer"
                    :src="viewerImageUrl"
                    :alt="imageAlt(selectedGalleryItem)"
                    :class="
                        zoomedIn
                            ? 'max-w-none cursor-zoom-out'
                            : 'max-h-[calc(100vh-15rem)] max-w-full cursor-zoom-in'
                    "
                    @click="zoomedIn = !zoomedIn"
                />
            </div>
            <div v-if="filteredGallery.length > 1" class="absolute left-4 top-1/2 -translate-y-1/2">
                <Button
                    circular
                    icon-only
                    :label="formatMessage(commonMessages.backButton)"
                    @click="changeImage(-1)"
                    ><LeftArrowIcon />
                </Button>
            </div>
            <div
                v-if="filteredGallery.length > 1"
                class="absolute right-4 top-1/2 -translate-y-1/2"
            >
                <Button
                    circular
                    icon-only
                    :label="formatMessage(commonMessages.nextButton)"
                    @click="changeImage(1)"
                    ><RightArrowIcon />
                </Button>
            </div>
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
                    <p
                        v-if="showBilingualTranslation(selectedGalleryItem, 'description')"
                        :class="translationClass"
                    >
                        {{ translationFor(selectedGalleryItem, 'description') }}
                    </p>
                    <div class="mt-1 flex items-center gap-2">
                        <CalendarIcon class="size-4 shrink-0" aria-hidden="true" />
                        {{ formatDate(new Date(selectedGalleryItem.image.created)) }}
                    </div>
                </div>
                <div class="flex flex-wrap items-center gap-2">
                    <Button @click="zoomedIn = !zoomedIn"
                        ><ContractIcon v-if="zoomedIn" />
                        <ExpandIcon v-else />
                        {{
                            formatMessage(
                                zoomedIn ? screenshotMessages.zoomOut : screenshotMessages.zoomIn,
                            )
                        }}
                    </Button>
                    <ButtonLink
                        target="_blank"
                        rel="noreferrer"
                        :href="selectedGalleryItem.image.raw_url ?? selectedGalleryItem.image.url"
                    >
                        <ExternalIcon />
                        {{ formatMessage(commonMessages.openInBrowserButton) }}
                    </ButtonLink>
                </div>
            </div>
        </template>
    </NewModal>
</template>

<style scoped>
.gallery-translation {
    margin: 0.25rem 0 0;
    animation: translation-float-in 0.5s ease-out both;
}

.gallery-translation--weakened {
    color: var(--color-text-tertiary);
}

.gallery-translation--blur {
    filter: blur(4px);
    opacity: 0.75;
    transition:
        filter 0.1s ease-in-out,
        opacity 0.1s ease-in-out;
}

.gallery-translation--blur:hover {
    filter: blur(0);
    opacity: 1;
}

.gallery-translation--blockquote {
    padding: 4px 0 4px 8px;
    border-left: 4px solid var(--color-brand);
}

.gallery-translation--dashed-line {
    text-decoration: underline dashed var(--color-brand);
    text-underline-offset: 5px;
}

.gallery-translation--border {
    padding: 2px 4px;
    border: 1px solid var(--color-brand);
    border-radius: 4px;
}

.gallery-translation--text-color {
    color: oklch(0.693 0.17 162.48);
}

.gallery-translation--background {
    padding: 2px 4px;
    border-radius: 4px;
    background-color: color-mix(in srgb, var(--color-brand) 15%, transparent);
}

@keyframes translation-float-in {
    from {
        opacity: 0;
        transform: translateY(12px);
    }
    to {
        opacity: 1;
        transform: translateY(0);
    }
}
</style>
