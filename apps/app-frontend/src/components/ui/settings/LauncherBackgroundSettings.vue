<template>
    <div class="flex flex-col gap-4 p-4 appearance-panel--divided">
        <div
            ref="backgroundPreviewRef"
            class="group relative h-44 cursor-pointer overflow-hidden rounded-lg border border-solid transition-colors"
            :class="
                isBackgroundDragActive
                    ? 'border-brand bg-surface-2'
                    : 'border-surface-4 bg-surface-1'
            "
            role="button"
            tabindex="0"
            :aria-label="formatMessage(messages.customBackgroundChoose)"
            @click="chooseCustomBackground"
            @keydown.enter.prevent="chooseCustomBackground"
            @keydown.space.prevent="chooseCustomBackground"
        >
            <div
                v-if="customBackgroundPreview"
                class="absolute -inset-10 bg-cover bg-center"
                :style="{
                    backgroundImage: `url(&quot;${customBackgroundPreview}&quot;)`,
                    filter: `blur(${settings.custom_background_blur}px)`,
                    opacity: settings.custom_background_opacity / 100,
                }"
            />
            <div class="custom-bg-preview-dim absolute inset-0" />
            <div class="relative flex h-full items-center justify-center">
                <div
                    v-if="!customBackgroundPreview"
                    class="flex flex-col items-center gap-2 text-[var(--color-text-tertiary)]"
                >
                    <ImageIcon class="size-8" />
                    <span class="font-semibold">{{
                        isBackgroundDragActive
                            ? formatMessage(messages.customBackgroundDropHint)
                            : formatMessage(messages.customBackgroundEmpty)
                    }}</span>
                    <span v-if="!isBackgroundDragActive" class="text-sm">
                        {{ formatMessage(messages.customBackgroundChooseOrDrop) }}
                    </span>
                </div>
                <div
                    v-else-if="isBackgroundDragActive"
                    class="custom-bg-preview-drag absolute inset-0 flex items-center justify-center"
                >
                    <span class="font-semibold text-[var(--color-text-primary)]">
                        {{ formatMessage(messages.customBackgroundDropHint) }}
                    </span>
                </div>
                <div
                    v-if="customBackgroundPreview && !isBackgroundDragActive"
                    class="custom-bg-preview-actions absolute inset-x-0 bottom-0 flex items-center justify-center gap-2 p-3 opacity-0 transition-opacity group-hover:opacity-100 group-focus-within:opacity-100"
                >
                    <HeadlessTooltip side="top">
                        <Button
                            type="base"
                            native-type="button"
                            data-testid="custom-background-replace"
                            @click.stop="chooseCustomBackground"
                        >
                            <UploadIcon />
                            {{ formatMessage(messages.customBackgroundReplace) }}
                        </Button>
                        <template #content>
                            {{ formatMessage(messages.customBackgroundChooseOrDrop) }}
                        </template>
                    </HeadlessTooltip>
                    <Button
                        type="outlined"
                        color="red"
                        native-type="button"
                        data-testid="custom-background-remove"
                        @click.stop="removeCustomBackground"
                    >
                        <TrashIcon />
                        {{ formatMessage(messages.customBackgroundRemove) }}
                    </Button>
                </div>
            </div>
        </div>

        <div v-if="customBackgroundPreview" class="grid gap-5 lg:grid-cols-2">
            <div class="flex flex-col gap-2">
                <h3 class="m-0 font-semibold text-[var(--color-text-primary)]">
                    {{ formatMessage(messages.customBackgroundBlur) }}
                </h3>
                <Slider
                    id="custom-background-blur"
                    v-model="settings.custom_background_blur"
                    :min="0"
                    :max="40"
                    :step="1"
                    unit="px"
                />
                <p class="m-0 text-sm text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.customBackgroundBlurDescription) }}
                </p>
            </div>
            <div class="flex flex-col gap-2">
                <h3 class="m-0 font-semibold text-[var(--color-text-primary)]">
                    {{ formatMessage(messages.customBackgroundOpacity) }}
                </h3>
                <Slider
                    id="custom-background-opacity"
                    v-model="settings.custom_background_opacity"
                    :min="10"
                    :max="100"
                    :step="5"
                    unit="%"
                />
                <p class="m-0 text-sm text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.customBackgroundOpacityDescription) }}
                </p>
            </div>
            <div class="flex flex-col gap-2">
                <h3 class="m-0 font-semibold text-[var(--color-text-primary)]">
                    {{ formatMessage(messages.customBackgroundComponentOpacity) }}
                </h3>
                <Slider
                    id="custom-background-component-opacity"
                    v-model="settings.custom_background_component_opacity"
                    :min="0"
                    :max="100"
                    :step="5"
                    unit="%"
                />
                <p class="m-0 text-sm text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.customBackgroundComponentOpacityDescription) }}
                </p>
            </div>
        </div>

        <ImageCropModal
            ref="customBackgroundCropModal"
            :title="formatMessage(messages.customBackgroundCropTitle)"
            :confirm-label="formatMessage(messages.customBackgroundCropConfirm)"
            output-mime-type="image/webp"
            :output-quality="0.9"
            :max-output-dimension="3840"
        />
    </div>
</template>

<script setup lang="ts">
import { ImageIcon, TrashIcon, UploadIcon } from '@modrinth/assets'
import {
    Button,
    defineMessages,
    type ImageCropAspectRatioOption,
    ImageCropModal,
    type ImageCropModalExposed,
    injectNotificationManager,
    Slider,
    useVIntl,
} from '@modrinth/ui'
import { convertFileSrc, invoke } from '@tauri-apps/api/core'
import { appDataDir, join } from '@tauri-apps/api/path'
import type { DragDropEvent } from '@tauri-apps/api/webview'
import { getCurrentWebview } from '@tauri-apps/api/webview'
import { open } from '@tauri-apps/plugin-dialog'
import { exists, mkdir, readFile, remove, writeFile } from '@tauri-apps/plugin-fs'
import { computed, onMounted, onUnmounted, ref } from 'vue'

import HeadlessTooltip from '@/components/ui/headless/HeadlessTooltip.vue'
import {
    CUSTOM_BACKGROUND_EXTENSIONS,
    type CustomBackgroundExtension,
    customBackgroundMimeType,
    extensionFromPath,
    isCroppableCustomBackground,
    isCustomBackgroundImagePath,
    isCustomBackgroundWithinByteLimit,
} from '@/helpers/custom-background'
import type { AppSettings } from '@/helpers/settings.ts'

type BackgroundSettings = Pick<
    AppSettings,
    | 'custom_background_path'
    | 'custom_background_blur'
    | 'custom_background_opacity'
    | 'custom_background_component_opacity'
>

const settings = defineModel<BackgroundSettings>('settings', { required: true })

const messages = defineMessages({
    customBackgroundEmpty: {
        id: 'app.appearance-settings.custom-background.empty',
        defaultMessage: 'No custom background selected',
    },
    customBackgroundChoose: {
        id: 'app.appearance-settings.custom-background.choose',
        defaultMessage: 'Choose image',
    },
    customBackgroundChooseOrDrop: {
        id: 'app.appearance-settings.custom-background.choose-or-drop',
        defaultMessage: 'Click to choose, or drop an image here',
    },
    customBackgroundDropHint: {
        id: 'app.appearance-settings.custom-background.drop-hint',
        defaultMessage: 'Drop image here',
    },
    customBackgroundReplace: {
        id: 'app.appearance-settings.custom-background.replace',
        defaultMessage: 'Replace image',
    },
    customBackgroundRemove: {
        id: 'app.appearance-settings.custom-background.remove',
        defaultMessage: 'Remove',
    },
    customBackgroundBlur: {
        id: 'app.appearance-settings.custom-background.blur',
        defaultMessage: 'Background blur',
    },
    customBackgroundBlurDescription: {
        id: 'app.appearance-settings.custom-background.blur-description',
        defaultMessage: 'Soften image details to keep launcher content easy to read.',
    },
    customBackgroundOpacity: {
        id: 'app.appearance-settings.custom-background.opacity',
        defaultMessage: 'Background visibility',
    },
    customBackgroundOpacityDescription: {
        id: 'app.appearance-settings.custom-background.opacity-description',
        defaultMessage: 'Control how strongly the image shows through the interface.',
    },
    customBackgroundComponentOpacity: {
        id: 'app.appearance-settings.custom-background.component-opacity',
        defaultMessage: 'Component opacity',
    },
    customBackgroundComponentOpacityDescription: {
        id: 'app.appearance-settings.custom-background.component-opacity-description',
        defaultMessage:
            'How opaque launcher components and chrome stay over a custom background. At 100% components are fully opaque; lower it to let the image show through.',
    },
    customBackgroundCropTitle: {
        id: 'app.appearance-settings.custom-background.crop-title',
        defaultMessage: 'Crop launcher background',
    },
    customBackgroundCropConfirm: {
        id: 'app.appearance-settings.custom-background.crop-confirm',
        defaultMessage: 'Use as background',
    },
    customBackgroundCropAspectWindow: {
        id: 'app.appearance-settings.custom-background.crop-aspect-window',
        defaultMessage: 'Current window',
    },
    customBackgroundAnimatedUnsupported: {
        id: 'app.appearance-settings.custom-background.animated-unsupported',
        defaultMessage:
            "Animated images aren't supported as launcher backgrounds yet. Choose a static image.",
    },
    customBackgroundTooLarge: {
        id: 'app.appearance-settings.custom-background.too-large',
        defaultMessage: 'This image is too large to use as a launcher background.',
    },
})

const { formatMessage } = useVIntl()
const { addNotification, handleError } = injectNotificationManager()

const customBackgroundCropModal = ref<ImageCropModalExposed>()
const customBackgroundBusy = ref(false)
const backgroundPreviewRef = ref<HTMLElement | null>(null)
const isBackgroundDragActive = ref(false)
let unlistenNativeBackgroundDrop: (() => void) | null = null

const customBackgroundPreview = computed(() =>
    settings.value.custom_background_path
        ? convertFileSrc(settings.value.custom_background_path)
        : null,
)

function buildAspectRatioOptions(): ImageCropAspectRatioOption[] {
    const width = window.innerWidth
    const height = window.innerHeight
    const windowRatio = width > 0 && height > 0 ? width / height : 16 / 9

    return [
        {
            id: 'window',
            label: formatMessage(messages.customBackgroundCropAspectWindow),
            ratio: windowRatio,
        },
        { id: '16:9', label: '16:9', ratio: 16 / 9 },
        { id: '16:10', label: '16:10', ratio: 16 / 10 },
    ]
}

async function storeCustomBackgroundBytes(bytes: Uint8Array, extension: 'png' | 'webp') {
    if (!isCustomBackgroundWithinByteLimit(bytes.byteLength)) {
        addNotification({
            type: 'error',
            title: formatMessage(messages.customBackgroundTooLarge),
        })
        return
    }

    const backgroundDirectory = await join(await appDataDir(), 'backgrounds')
    const suffix =
        globalThis.crypto?.randomUUID?.().slice(0, 8) ?? Math.random().toString(36).slice(2, 10)
    const storedPath = await join(
        backgroundDirectory,
        `launcher-background-${Date.now()}-${suffix}.${extension}`,
    )
    const previousPath = settings.value.custom_background_path

    await mkdir(backgroundDirectory, { recursive: true })
    try {
        await writeFile(storedPath, bytes)
    } catch (error) {
        try {
            if (await exists(storedPath)) await remove(storedPath)
        } catch {
            // Best-effort cleanup of a partial write.
        }
        throw error
    }

    settings.value.custom_background_path = storedPath

    if (previousPath && previousPath !== storedPath && (await exists(previousPath))) {
        try {
            await remove(previousPath)
        } catch (error) {
            console.warn('Failed to remove previous custom background', error)
        }
    }
}

async function applyCustomBackground(bytes: Uint8Array, extension: CustomBackgroundExtension) {
    const mimeType = customBackgroundMimeType(extension)
    if (!isCroppableCustomBackground(mimeType, bytes)) {
        addNotification({
            type: 'warning',
            title: formatMessage(messages.customBackgroundAnimatedUnsupported),
        })
        return
    }

    const modal = customBackgroundCropModal.value
    if (!modal) return

    const result = await modal.show(new Blob([bytes as BlobPart], { type: mimeType }), {
        aspectRatioOptions: buildAspectRatioOptions(),
        defaultAspectRatioId: 'window',
    })
    if (!result) return

    await storeCustomBackgroundBytes(result.bytes, result.extension)
}

async function chooseCustomBackground() {
    if (customBackgroundBusy.value) return

    customBackgroundBusy.value = true
    try {
        const selectedPath = await open({
            multiple: false,
            filters: [
                {
                    name: 'Image',
                    extensions: [...CUSTOM_BACKGROUND_EXTENSIONS],
                },
            ],
        })

        if (!selectedPath || Array.isArray(selectedPath)) return

        const extension = extensionFromPath(selectedPath)
        if (!extension) return

        await applyCustomBackground(await readFile(selectedPath), extension)
    } catch (error) {
        handleError(error)
    } finally {
        customBackgroundBusy.value = false
    }
}

async function storeCustomBackgroundFromDroppedPath(path: string) {
    if (customBackgroundBusy.value) return
    const extension = extensionFromPath(path)
    if (!extension) return

    customBackgroundBusy.value = true
    try {
        const data = await invoke<ArrayBuffer>('plugin:files|file_read_dragged_file', { path })
        await applyCustomBackground(new Uint8Array(data), extension)
    } catch (error) {
        handleError(error)
    } finally {
        customBackgroundBusy.value = false
    }
}

async function removeCustomBackground() {
    if (customBackgroundBusy.value) return

    const backgroundPath = settings.value.custom_background_path
    if (!backgroundPath) return

    try {
        if (await exists(backgroundPath)) await remove(backgroundPath)
        settings.value.custom_background_path = null
    } catch (error) {
        // Keep the setting so the preview still points at the file that failed
        // to delete; otherwise the launcher shows no background and the file
        // becomes an orphan.
        handleError(error)
    }
}

function pointInBackgroundPreview(position: { x: number; y: number }) {
    const rect = backgroundPreviewRef.value?.getBoundingClientRect()
    if (!rect) return false
    const scale = window.devicePixelRatio || 1
    const x = position.x / scale
    const y = position.y / scale
    return x >= rect.left && x <= rect.right && y >= rect.top && y <= rect.bottom
}

async function setupNativeBackgroundDrop() {
    try {
        unlistenNativeBackgroundDrop = await getCurrentWebview().onDragDropEvent(
            (event: { payload: DragDropEvent }) => {
                const payload = event.payload
                if (payload.type === 'leave') {
                    isBackgroundDragActive.value = false
                    return
                }

                const paths = 'paths' in payload ? payload.paths : []
                const path = paths.find((item) => isCustomBackgroundImagePath(item))
                const inside = pointInBackgroundPreview(payload.position)

                if (payload.type === 'enter' || payload.type === 'over') {
                    isBackgroundDragActive.value = Boolean(path && inside)
                    return
                }

                if (payload.type === 'drop') {
                    isBackgroundDragActive.value = false
                    if (path && inside) void storeCustomBackgroundFromDroppedPath(path)
                }
            },
        )
    } catch (error) {
        console.warn('Failed to set up native drop handler for launcher background', error)
    }
}

onMounted(() => {
    void setupNativeBackgroundDrop()
})

onUnmounted(() => {
    unlistenNativeBackgroundDrop?.()
})
</script>

<style scoped lang="scss">
.appearance-panel--divided {
    border-bottom: 1px solid var(--settings-divider, var(--surface-4));
}

.custom-bg-preview-dim {
    background-color: color-mix(
        in srgb,
        var(--surface-1) var(--opacity-ratio-keep-35),
        transparent
    );
}

.custom-bg-preview-drag {
    background-color: color-mix(
        in srgb,
        var(--surface-1) var(--opacity-ratio-keep-70),
        transparent
    );
}

.custom-bg-preview-actions {
    background-color: color-mix(
        in srgb,
        var(--surface-1) var(--opacity-ratio-keep-80),
        transparent
    );
}
</style>
