<template>
    <div class="flex flex-col gap-4 p-4 appearance-panel--divided">
        <div
            ref="backgroundPreviewRef"
            class="group relative h-32 w-full cursor-pointer overflow-hidden rounded-lg border border-solid transition-colors"
            :class="
                isBackgroundDragActive
                    ? 'border-brand bg-surface-2'
                    : 'border-surface-4 bg-surface-1'
            "
            role="button"
            tabindex="0"
            :aria-busy="customBackgroundConverting"
            :aria-label="
                formatMessage(
                    customBackgroundPreview
                        ? messages.customBackgroundAdjust
                        : messages.customBackgroundChoose,
                )
            "
            @pointerenter="schedulePreviewOpen"
            @pointermove="schedulePreviewOpen"
            @pointerleave="schedulePreviewClose"
            @click="handleBackgroundPreviewClick"
            @keydown.enter.prevent="handleBackgroundPreviewClick"
            @keydown.space.prevent="handleBackgroundPreviewClick"
        >
            <div
                v-if="customBackgroundPreview"
                class="absolute -inset-10 bg-cover bg-center"
                :style="previewSurfaceStyle"
            />
            <div class="custom-bg-preview-dim absolute inset-0" />
            <div
                v-if="customBackgroundConverting"
                data-testid="custom-background-converting"
                aria-hidden="true"
                class="custom-bg-preview-converting absolute inset-0 z-20 flex flex-col items-center justify-center gap-1 px-4 text-center"
            >
                <SpinnerIcon class="mb-1 size-6 animate-spin text-brand" />
                <span class="text-sm font-semibold text-[var(--color-text-primary)]">
                    {{ formatMessage(messages.customBackgroundConverting) }}
                </span>
                <span class="text-xs text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.customBackgroundConvertingHint) }}
                </span>
            </div>
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
            </div>
            <Button
                v-if="
                    customBackgroundPreview &&
                    !isBackgroundDragActive &&
                    !customBackgroundConverting
                "
                type="base"
                icon-only
                circular
                :label="formatMessage(messages.previewExpand)"
                data-testid="custom-background-expand"
                class="absolute right-2 top-2 z-10"
                @click.stop="openPreviewOverlay(true)"
            >
                <MaximizeIcon />
            </Button>
        </div>

        <div v-if="customBackgroundPreview" class="flex flex-wrap items-center justify-end gap-2">
            <HeadlessTooltip side="top">
                <Button
                    type="base"
                    native-type="button"
                    data-testid="custom-background-replace"
                    :disabled="customBackgroundConverting"
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
                :disabled="customBackgroundConverting"
                @click.stop="removeCustomBackground"
            >
                <TrashIcon />
                {{ formatMessage(messages.customBackgroundRemove) }}
            </Button>
        </div>

        <Teleport to="body">
            <div
                v-if="previewOverlayOpen"
                class="fixed inset-0 z-[90] flex items-center justify-center bg-black/50 p-6"
                data-testid="custom-background-overlay"
                @click.self="closePreviewOverlay(true)"
                @keydown.esc.stop.prevent="closePreviewOverlay(true)"
            >
                <div
                    ref="overlayPanelRef"
                    tabindex="-1"
                    class="relative overflow-hidden rounded-2xl border border-surface-5 bg-surface-3 shadow-2xl outline-none"
                    :style="overlayStyle"
                    data-testid="custom-background-overlay-panel"
                    @pointerenter="cancelPreviewClose"
                    @pointerleave="schedulePreviewClose"
                    @keydown.esc.stop.prevent="closePreviewOverlay(true)"
                >
                    <div
                        v-if="customBackgroundPreview"
                        class="absolute -inset-10 bg-cover bg-center"
                        :style="previewSurfaceStyle"
                    />
                    <div class="custom-bg-preview-dim absolute inset-0" />
                    <div
                        class="absolute inset-0 cursor-pointer"
                        role="button"
                        tabindex="0"
                        :aria-label="formatMessage(messages.customBackgroundAdjust)"
                        @click="handleBackgroundPreviewClick"
                        @keydown.enter.prevent="handleBackgroundPreviewClick"
                        @keydown.space.prevent="handleBackgroundPreviewClick"
                    />
                    <div class="absolute right-3 top-3 z-10 flex items-center gap-2">
                        <Button
                            type="base"
                            size="sm"
                            data-testid="custom-background-pin"
                            @click.stop="togglePreviewPinned"
                        >
                            <PinIcon />
                            {{
                                formatMessage(
                                    previewPinned ? messages.previewUnpin : messages.previewPin,
                                )
                            }}
                        </Button>
                        <Button
                            type="base"
                            icon-only
                            circular
                            :label="formatMessage(messages.previewClose)"
                            data-testid="custom-background-overlay-close"
                            @click.stop="closePreviewOverlay(true)"
                        >
                            <XIcon />
                        </Button>
                    </div>
                    <div
                        class="custom-bg-preview-actions absolute inset-x-0 bottom-0 z-10 flex items-center justify-center gap-2 p-3"
                    >
                        <Button
                            type="base"
                            data-testid="custom-background-overlay-replace"
                            @click.stop="replaceFromOverlay"
                        >
                            <UploadIcon />
                            {{ formatMessage(messages.customBackgroundReplace) }}
                        </Button>
                        <Button
                            type="outlined"
                            color="red"
                            data-testid="custom-background-overlay-remove"
                            @click.stop="removeFromOverlay"
                        >
                            <TrashIcon />
                            {{ formatMessage(messages.customBackgroundRemove) }}
                        </Button>
                    </div>
                </div>
            </div>
        </Teleport>

        <p class="sr-only" role="status" aria-live="polite">
            {{
                customBackgroundConverting ? formatMessage(messages.customBackgroundConverting) : ''
            }}
        </p>

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
import {
    ImageIcon,
    MaximizeIcon,
    PinIcon,
    SpinnerIcon,
    TrashIcon,
    UploadIcon,
    XIcon,
} from '@modrinth/assets'
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
import { computed, nextTick, onMounted, onUnmounted, ref } from 'vue'

import HeadlessTooltip from '@/components/ui/headless/HeadlessTooltip.vue'
import {
    convertAnimatedBackground,
    isBackgroundConversionError,
} from '@/helpers/background-conversion'
import {
    classifyCustomBackground,
    CUSTOM_BACKGROUND_EXTENSIONS,
    type CustomBackgroundExtension,
    customBackgroundMimeType,
    detectCustomBackgroundExtension,
    extensionFromPath,
    isCroppableCustomBackground,
    isCustomBackgroundWithinByteLimit,
    MAX_CUSTOM_BACKGROUND_BYTES,
    MAX_PRESERVED_BACKGROUND_BYTES,
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
    customBackgroundAdjust: {
        id: 'app.appearance-settings.custom-background.adjust',
        defaultMessage: 'Adjust background crop',
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
        defaultMessage: "This image format isn't supported as a launcher background.",
    },
    customBackgroundConverting: {
        id: 'app.appearance-settings.custom-background.converting',
        defaultMessage: 'Converting animated background…',
    },
    customBackgroundConvertingHint: {
        id: 'app.appearance-settings.custom-background.converting-hint',
        defaultMessage: 'This can take a few seconds for longer animations.',
    },
    customBackgroundConversionFailed: {
        id: 'app.appearance-settings.custom-background.conversion-failed',
        defaultMessage: 'The animated background could not be converted.',
    },
    customBackgroundBackendUnavailable: {
        id: 'app.appearance-settings.custom-background.backend-unavailable',
        defaultMessage: 'AVIF and MP4 backgrounds need FFmpeg support, which is not available yet.',
    },
    customBackgroundPickerMedia: {
        id: 'app.appearance-settings.custom-background.picker-media',
        defaultMessage: 'Image or video',
    },
    customBackgroundPickerAllFiles: {
        id: 'app.appearance-settings.custom-background.picker-all-files',
        defaultMessage: 'All files',
    },
    previewExpand: {
        id: 'app.appearance-settings.custom-background.preview-expand',
        defaultMessage: 'Expand preview',
    },
    previewPin: {
        id: 'app.appearance-settings.custom-background.preview-pin',
        defaultMessage: 'Keep open',
    },
    previewUnpin: {
        id: 'app.appearance-settings.custom-background.preview-unpin',
        defaultMessage: 'Unpin',
    },
    previewClose: {
        id: 'app.appearance-settings.custom-background.preview-close',
        defaultMessage: 'Close preview',
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
const customBackgroundConverting = ref(false)
const backgroundPreviewRef = ref<HTMLElement | null>(null)
const isBackgroundDragActive = ref(false)
let unlistenNativeBackgroundDrop: (() => void) | null = null

const customBackgroundPreview = computed(() =>
    settings.value.custom_background_path
        ? convertFileSrc(settings.value.custom_background_path)
        : null,
)

function currentWindowAspectRatio() {
    const width = window.innerWidth
    const height = window.innerHeight
    return width > 0 && height > 0 ? width / height : 16 / 9
}

const previewAspectRatio = ref(currentWindowAspectRatio())

function updatePreviewAspectRatio() {
    previewAspectRatio.value = currentWindowAspectRatio()
}

const previewSurfaceStyle = computed(() =>
    customBackgroundPreview.value
        ? {
              backgroundImage: `url("${customBackgroundPreview.value}")`,
              filter: `blur(${settings.value.custom_background_blur}px)`,
              opacity: settings.value.custom_background_opacity / 100,
          }
        : {},
)

const overlayStyle = computed(() => ({
    width: `min(90vw, ${(85 * previewAspectRatio.value).toFixed(3)}vh)`,
    maxWidth: '90rem',
    aspectRatio: String(previewAspectRatio.value),
}))

const previewOverlayOpen = ref(false)
const previewPinned = ref(false)
const overlayPanelRef = ref<HTMLElement | null>(null)
let previewOpenTimer: ReturnType<typeof setTimeout> | undefined
let previewCloseTimer: ReturnType<typeof setTimeout> | undefined

function clearPreviewTimers() {
    clearTimeout(previewOpenTimer)
    clearTimeout(previewCloseTimer)
    previewOpenTimer = undefined
    previewCloseTimer = undefined
}

const PREVIEW_HOVER_OPEN_DELAY_MS = 1_000
const PREVIEW_HOVER_CLOSE_DELAY_MS = 280

function schedulePreviewOpen() {
    if (
        !customBackgroundPreview.value ||
        customBackgroundConverting.value ||
        previewOverlayOpen.value ||
        previewPinned.value
    )
        return
    clearTimeout(previewOpenTimer)
    clearTimeout(previewCloseTimer)
    previewOpenTimer = setTimeout(() => openPreviewOverlay(false), PREVIEW_HOVER_OPEN_DELAY_MS)
}

function schedulePreviewClose() {
    clearTimeout(previewOpenTimer)
    if (previewPinned.value) return
    clearTimeout(previewCloseTimer)
    previewCloseTimer = setTimeout(() => closePreviewOverlay(false), PREVIEW_HOVER_CLOSE_DELAY_MS)
}

function cancelPreviewClose() {
    clearTimeout(previewCloseTimer)
}

function animatePreviewOverlayIn(panel: HTMLElement) {
    const source = backgroundPreviewRef.value
    if (!source) return

    const from = source.getBoundingClientRect()
    const to = panel.getBoundingClientRect()
    if (to.width === 0 || to.height === 0) return

    const dx = from.left + from.width / 2 - (to.left + to.width / 2)
    const dy = from.top + from.height / 2 - (to.top + to.height / 2)
    const scale = Math.min(1, from.width / to.width)

    panel.style.transition = 'none'
    panel.style.transform = `translate(${dx}px, ${dy}px) scale(${scale})`
    panel.style.opacity = '0'
    panel.getBoundingClientRect()

    requestAnimationFrame(() => {
        if (!previewOverlayOpen.value) return
        panel.style.transition =
            'transform 320ms cubic-bezier(0.16, 1.2, 0.3, 1), opacity 180ms ease'
        panel.style.transform = 'translate(0, 0) scale(1)'
        panel.style.opacity = '1'
    })
}

function openPreviewOverlay(pin: boolean) {
    if (!customBackgroundPreview.value) return
    clearPreviewTimers()
    if (pin) previewPinned.value = true
    if (previewOverlayOpen.value) return

    previewOverlayOpen.value = true
    void nextTick(() => {
        const panel = overlayPanelRef.value
        if (!panel) return
        panel.focus()
        animatePreviewOverlayIn(panel)
    })
}

function closePreviewOverlay(force = false) {
    if (!force && previewPinned.value) return
    clearPreviewTimers()
    previewPinned.value = false
    previewOverlayOpen.value = false
}

function togglePreviewPinned() {
    previewPinned.value = !previewPinned.value
}

function replaceFromOverlay() {
    closePreviewOverlay(true)
    void chooseCustomBackground()
}

function removeFromOverlay() {
    closePreviewOverlay(true)
    void removeCustomBackground()
}

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

async function buildCustomBackgroundPath(extension: CustomBackgroundExtension) {
    const backgroundDirectory = await join(await appDataDir(), 'backgrounds')
    const suffix =
        globalThis.crypto?.randomUUID?.().slice(0, 8) ?? Math.random().toString(36).slice(2, 10)
    await mkdir(backgroundDirectory, { recursive: true })
    return join(backgroundDirectory, `launcher-background-${Date.now()}-${suffix}.${extension}`)
}

/**
 * The pre-compression import lives next to its rendered background with a
 * `.source` suffix. Deriving it from the background path keeps the settings
 * schema unchanged while making "click to re-crop" use the original asset.
 */
function sourcePathForBackground(backgroundPath: string) {
    return backgroundPath.replace(/\.[^./\\]+$/, '.source')
}

async function adoptCustomBackgroundPath(storedPath: string) {
    const previousPath = settings.value.custom_background_path
    const previousSourcePath = previousPath ? sourcePathForBackground(previousPath) : null
    settings.value.custom_background_path = storedPath

    if (previousPath && previousPath !== storedPath && (await exists(previousPath))) {
        try {
            await remove(previousPath)
        } catch (error) {
            console.warn('Failed to remove previous custom background', error)
        }
    }

    if (
        previousSourcePath &&
        previousSourcePath !== sourcePathForBackground(storedPath) &&
        (await exists(previousSourcePath))
    ) {
        try {
            await remove(previousSourcePath)
        } catch (error) {
            console.warn('Failed to remove previous background source', error)
        }
    }
}

async function storeCustomBackgroundBytes(
    bytes: Uint8Array,
    extension: CustomBackgroundExtension,
    options: { bypassLimit?: boolean; sourceBytes?: Uint8Array } = {},
) {
    if (!options.bypassLimit && !isCustomBackgroundWithinByteLimit(bytes.byteLength)) {
        addNotification({
            type: 'error',
            title: formatMessage(messages.customBackgroundTooLarge),
        })
        return
    }

    const storedPath = await buildCustomBackgroundPath(extension)
    const sourcePath = sourcePathForBackground(storedPath)
    if (options.sourceBytes) {
        try {
            await writeFile(sourcePath, options.sourceBytes)
        } catch (error) {
            try {
                if (await exists(sourcePath)) await remove(sourcePath)
            } catch {
                // Best-effort cleanup of a partial source write.
            }
            throw error
        }
    }
    try {
        await writeFile(storedPath, bytes)
    } catch (error) {
        try {
            if (await exists(storedPath)) await remove(storedPath)
            if (options.sourceBytes && (await exists(sourcePath))) await remove(sourcePath)
        } catch {
            // Best-effort cleanup of a partial write.
        }
        throw error
    }

    await adoptCustomBackgroundPath(storedPath)
}

async function convertAnimatedCustomBackground(
    bytes: Uint8Array,
    extension: CustomBackgroundExtension,
    sourcePath: string,
) {
    const modal = customBackgroundCropModal.value
    if (!modal) return

    const mimeType = customBackgroundMimeType(extension)
    const selection = await modal.showCrop(new Blob([bytes as BlobPart], { type: mimeType }), {
        aspectRatioOptions: buildAspectRatioOptions(),
        defaultAspectRatioId: 'window',
        maxOutputDimension: 1920,
        animated: true,
    })
    if (!selection) return

    const preserveOriginalQuality = selection.preserveOriginalQuality
    const outputPath = await buildCustomBackgroundPath('webp')
    const sourceCopyPath = sourcePathForBackground(outputPath)
    closePreviewOverlay(true)
    customBackgroundConverting.value = true
    try {
        await writeFile(sourceCopyPath, bytes)
        const result = await convertAnimatedBackground({
            inputPath: sourcePath,
            outputPath,
            crop: selection.normalizedCrop,
            outputWidth: selection.outputWidth,
            outputHeight: selection.outputHeight,
            quality: preserveOriginalQuality ? 90 : 75,
            maxFrames: preserveOriginalQuality ? 1000 : 240,
            maxDurationMs: preserveOriginalQuality ? 60_000 : 15_000,
            maxOutputBytes: preserveOriginalQuality
                ? MAX_PRESERVED_BACKGROUND_BYTES
                : MAX_CUSTOM_BACKGROUND_BYTES,
            allowOversizedSource: selection.oversizedSource,
            preserveOriginalQuality,
            backend: 'auto',
        })
        await adoptCustomBackgroundPath(result.path)
    } catch (error) {
        try {
            if (await exists(outputPath)) await remove(outputPath)
            if (await exists(sourceCopyPath)) await remove(sourceCopyPath)
        } catch {
            // Best-effort cleanup of a failed conversion.
        }

        if (isBackgroundConversionError(error)) {
            const title =
                error.code === 'backend_unavailable'
                    ? messages.customBackgroundBackendUnavailable
                    : error.code === 'too_large' || error.code === 'output_too_large'
                      ? messages.customBackgroundTooLarge
                      : messages.customBackgroundConversionFailed
            addNotification({ type: 'error', title: formatMessage(title) })
        } else {
            handleError(error)
        }
    } finally {
        customBackgroundConverting.value = false
    }
}

async function applyCustomBackground(
    bytes: Uint8Array,
    extension: CustomBackgroundExtension,
    sourcePath: string,
) {
    const kind = classifyCustomBackground(extension, bytes)

    if (kind === 'animated-image') {
        await convertAnimatedCustomBackground(bytes, extension, sourcePath)
        return
    }

    if (kind === 'animated-avif' || kind === 'video') {
        addNotification({
            type: 'warning',
            title: formatMessage(messages.customBackgroundBackendUnavailable),
        })
        return
    }

    if (kind !== 'static') {
        notifyUnsupportedBackground()
        return
    }

    const mimeType = customBackgroundMimeType(extension)
    if (!isCroppableCustomBackground(mimeType, bytes)) {
        notifyUnsupportedBackground()
        return
    }

    const modal = customBackgroundCropModal.value
    if (!modal) return

    const result = await modal.show(new Blob([bytes as BlobPart], { type: mimeType }), {
        aspectRatioOptions: buildAspectRatioOptions(),
        defaultAspectRatioId: 'window',
    })
    if (!result) return

    await storeCustomBackgroundBytes(result.bytes, result.extension, {
        bypassLimit: result.preserveOriginalQuality === true,
        sourceBytes: bytes,
    })
}

function notifyUnsupportedBackground() {
    addNotification({
        type: 'warning',
        title: formatMessage(messages.customBackgroundAnimatedUnsupported),
    })
}

async function handleBackgroundPreviewClick() {
    if (customBackgroundBusy.value) return
    closePreviewOverlay(true)

    const backgroundPath = settings.value.custom_background_path
    if (!backgroundPath) {
        await chooseCustomBackground()
        return
    }

    const sourcePath = sourcePathForBackground(backgroundPath)
    customBackgroundBusy.value = true
    try {
        const readPath = (await exists(sourcePath)) ? sourcePath : backgroundPath
        const bytes = await readFile(readPath)
        const extension = detectCustomBackgroundExtension(bytes, extensionFromPath(readPath))
        if (!extension) {
            notifyUnsupportedBackground()
            return
        }
        await applyCustomBackground(bytes, extension, readPath)
    } catch (error) {
        handleError(error)
    } finally {
        customBackgroundBusy.value = false
    }
}

async function chooseCustomBackground() {
    if (customBackgroundBusy.value) return
    closePreviewOverlay(true)

    customBackgroundBusy.value = true
    try {
        const selectedPath = await open({
            multiple: false,
            filters: [
                {
                    name: formatMessage(messages.customBackgroundPickerMedia),
                    extensions: [...CUSTOM_BACKGROUND_EXTENSIONS],
                },
                {
                    name: formatMessage(messages.customBackgroundPickerAllFiles),
                    extensions: ['*'],
                },
            ],
        })

        if (!selectedPath || Array.isArray(selectedPath)) return

        const bytes = await readFile(selectedPath)
        const extension = detectCustomBackgroundExtension(bytes, extensionFromPath(selectedPath))
        if (!extension) {
            notifyUnsupportedBackground()
            return
        }

        await applyCustomBackground(bytes, extension, selectedPath)
    } catch (error) {
        handleError(error)
    } finally {
        customBackgroundBusy.value = false
    }
}

async function storeCustomBackgroundFromDroppedPath(path: string) {
    if (customBackgroundBusy.value) return

    customBackgroundBusy.value = true
    try {
        const data = await invoke<ArrayBuffer>('plugin:files|file_read_dragged_file', { path })
        const bytes = new Uint8Array(data)
        const extension = detectCustomBackgroundExtension(bytes, extensionFromPath(path))
        if (!extension) {
            notifyUnsupportedBackground()
            return
        }
        await applyCustomBackground(bytes, extension, path)
    } catch (error) {
        handleError(error)
    } finally {
        customBackgroundBusy.value = false
    }
}

async function removeCustomBackground() {
    if (customBackgroundBusy.value) return
    closePreviewOverlay(true)

    const backgroundPath = settings.value.custom_background_path
    if (!backgroundPath) return

    try {
        const sourcePath = sourcePathForBackground(backgroundPath)
        if (await exists(sourcePath)) await remove(sourcePath)
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
                const path = paths[0]
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
    window.addEventListener('resize', updatePreviewAspectRatio)
    void setupNativeBackgroundDrop()
})

onUnmounted(() => {
    clearPreviewTimers()
    window.removeEventListener('resize', updatePreviewAspectRatio)
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

.custom-bg-preview-converting {
    background-color: color-mix(
        in srgb,
        var(--surface-1) var(--opacity-ratio-keep-90),
        transparent
    );
    backdrop-filter: blur(2px);
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
