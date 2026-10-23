<script setup lang="ts">
import RefreshCwIcon from '@modrinth/assets/icons/refresh-cw.svg?component'
import SaveIcon from '@modrinth/assets/icons/save.svg?component'
import SpinnerIcon from '@modrinth/assets/icons/spinner.svg?component'
import UploadIcon from '@modrinth/assets/icons/upload.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import { type MessageDescriptor, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { injectNotificationManager } from '@modrinth/ui/src/providers/web-notifications.ts'
import { computed, ref, useTemplateRef } from 'vue'

import { builtInInstanceIcons, modrinth3DInstanceIcons } from '../../../lib/instance-icons'
import PreviewModal from '../../../overlays/PreviewModal.vue'
import { iconBackgroundMessages, iconPickerMessages as messages } from './messages.extra'
import { useSimulatedInvoke } from './state'

/**
 * Port of `apps/app-frontend/src/components/ui/modal/InstanceIconPickerModal.vue`
 * (the picker `filePicker.pickInstanceIcon` opens). The generated icon is drawn on
 * the same canvas; instead of `cache_icon` writing it to the app's cache folder,
 * the preview keeps the image in memory for the life of the window.
 */
export interface PickedIcon {
    path: string
    previewUrl: string
    frameless?: boolean
}

interface IconBackground {
    id: string
    name: MessageDescriptor
    colors: [string, string] | null
    style: Record<string, string>
}

function background(
    id: string,
    name: MessageDescriptor,
    top: string,
    bottom: string,
): IconBackground {
    return {
        id,
        name,
        colors: [top, bottom],
        style: { backgroundImage: `linear-gradient(145deg, ${top}, ${bottom})` },
    }
}

const backgrounds: IconBackground[] = [
    {
        id: 'transparent',
        name: iconBackgroundMessages.transparent,
        colors: null,
        style: {
            backgroundColor: '#ffffff',
            backgroundImage:
                'linear-gradient(45deg, #d7d9de 25%, transparent 25%), linear-gradient(-45deg, #d7d9de 25%, transparent 25%), linear-gradient(45deg, transparent 75%, #d7d9de 75%), linear-gradient(-45deg, transparent 75%, #d7d9de 75%)',
            backgroundPosition: '0 0, 0 8px, 8px -8px, -8px 0',
            backgroundSize: '16px 16px',
        },
    },
    background('grass', iconBackgroundMessages.grass, '#7fc95b', '#2f7d32'),
    background('ocean', iconBackgroundMessages.ocean, '#55b7ff', '#3157c8'),
    background('amethyst', iconBackgroundMessages.amethyst, '#c084fc', '#6d28d9'),
    background('sunset', iconBackgroundMessages.sunset, '#ffba52', '#e94b64'),
    background('cherry', iconBackgroundMessages.cherry, '#ff9ec4', '#b93670'),
    background('nether', iconBackgroundMessages.nether, '#ef5a46', '#6d1717'),
    background('slime', iconBackgroundMessages.slime, '#b6ee55', '#44972f'),
    background('deep-dark', iconBackgroundMessages.deepDark, '#245369', '#0c1f2b'),
    background('stone', iconBackgroundMessages.stone, '#aeb5bd', '#525b66'),
    background('midnight', iconBackgroundMessages.midnight, '#4d5f8f', '#171b2d'),
]

const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()
const simulate = useSimulatedInvoke()
const modal = useTemplateRef<InstanceType<typeof PreviewModal>>('modal')
const fileInput = useTemplateRef<HTMLInputElement>('fileInput')
const iconOptions = [...builtInInstanceIcons, ...modrinth3DInstanceIcons]
const iconGroups = [
    {
        id: 'original',
        name: messages.groupOriginal,
        icons: builtInInstanceIcons,
    },
    {
        id: 'modrinth-3d',
        name: messages.groupModrinth3d,
        icons: modrinth3DInstanceIcons,
    },
]

const selectedBackgroundId = ref(backgrounds[0].id)
const selectedIconId = ref(builtInInstanceIcons[0].id)
const saving = ref(false)
let resolveSelection: ((selection: PickedIcon | null) => void) | null = null

const selectedBackground = computed(
    () => backgrounds.find((item) => item.id === selectedBackgroundId.value) ?? backgrounds[0],
)
const selectedIcon = computed(
    () => iconOptions.find((icon) => icon.id === selectedIconId.value) ?? builtInInstanceIcons[0],
)

function finish(selection: PickedIcon | null) {
    const resolve = resolveSelection
    resolveSelection = null
    modal.value?.hide()
    resolve?.(selection)
}

function handleHide() {
    saving.value = false
    if (resolveSelection) {
        resolveSelection(null)
        resolveSelection = null
    }
}

function surpriseMe() {
    if (backgrounds.length > 1) {
        const candidates = backgrounds.filter((item) => item.id !== selectedBackgroundId.value)
        selectedBackgroundId.value = candidates[Math.floor(Math.random() * candidates.length)].id
    }
    if (iconOptions.length > 1) {
        const candidates = iconOptions.filter((icon) => icon.id !== selectedIconId.value)
        selectedIconId.value = candidates[Math.floor(Math.random() * candidates.length)].id
    }
}

function loadImage(url: string): Promise<HTMLImageElement> {
    return new Promise((resolve, reject) => {
        const image = new Image()
        image.onload = () => resolve(image)
        image.onerror = () => reject(new Error(formatMessage(messages.loadError)))
        image.src = url
    })
}

function canvasToBlob(canvas: HTMLCanvasElement): Promise<Blob> {
    return new Promise((resolve, reject) => {
        canvas.toBlob((blob) => {
            if (blob) resolve(blob)
            else reject(new Error(formatMessage(messages.loadError)))
        }, 'image/png')
    })
}

async function renderGeneratedIcon(): Promise<Blob> {
    const colors = selectedBackground.value.colors
    if (!colors) {
        const response = await fetch(selectedIcon.value.url)
        if (!response.ok) throw new Error(formatMessage(messages.loadError))
        return await response.blob()
    }

    const size = 512
    const canvas = document.createElement('canvas')
    canvas.width = size
    canvas.height = size
    const context = canvas.getContext('2d')
    if (!context) throw new Error(formatMessage(messages.loadError))

    const gradient = context.createLinearGradient(0, 0, size, size)
    gradient.addColorStop(0, colors[0])
    gradient.addColorStop(1, colors[1])
    context.fillStyle = gradient
    context.fillRect(0, 0, size, size)

    const symbol = await loadImage(selectedIcon.value.url)
    const maxSymbolSize = size * 0.72
    const scale = Math.min(
        maxSymbolSize / symbol.naturalWidth,
        maxSymbolSize / symbol.naturalHeight,
    )
    const width = symbol.naturalWidth * scale
    const height = symbol.naturalHeight * scale
    context.drawImage(symbol, (size - width) / 2, (size - height) / 2, width, height)

    return await canvasToBlob(canvas)
}

async function saveGeneratedIcon() {
    if (saving.value) return
    saving.value = true
    try {
        const transparent = selectedBackgroundId.value === 'transparent'
        // A bundled icon on the transparent background is the bundled file itself.
        const path = transparent
            ? selectedIcon.value.url
            : URL.createObjectURL(await renderGeneratedIcon())
        await simulate(160)
        finish({
            path,
            previewUrl: path,
            frameless: transparent,
        })
    } catch (error) {
        handleError(error)
    } finally {
        saving.value = false
    }
}

/** `pickImage()`: the system image picker; the browser's file picker stands in for it. */
function selectUploadedIcon() {
    fileInput.value?.click()
}

function handleFileInput(event: Event) {
    const input = event.target as HTMLInputElement
    const file = input.files?.[0]
    input.value = ''
    if (!file) return
    const url = URL.createObjectURL(file)
    finish({ path: url, previewUrl: url })
}

function show(): Promise<PickedIcon | null> {
    resolveSelection?.(null)
    const modalInstance = modal.value
    if (!modalInstance) return Promise.resolve(null)

    return new Promise((resolve) => {
        resolveSelection = resolve
        modalInstance.show()
    })
}

defineExpose({ show })
</script>

<template>
    <PreviewModal
        ref="modal"
        :header="formatMessage(messages.title)"
        :on-hide="handleHide"
        width="760px"
        max-width="760px"
        scrollable
    >
        <div class="grid grid-cols-[220px_minmax(0,1fr)] gap-6">
            <div class="flex flex-col items-center gap-4">
                <div
                    class="flex aspect-square w-44 items-center justify-center overflow-hidden rounded-3xl shadow-lg"
                    :style="selectedBackground.style"
                >
                    <img :src="selectedIcon.url" alt="" class="h-[72%] w-[72%] object-contain" />
                </div>
                <p class="m-0 text-center text-sm text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.description) }}
                </p>
                <Button type="outlined" :disabled="saving" @click="surpriseMe"
                    ><RefreshCwIcon />
                    {{ formatMessage(messages.surpriseMe) }}
                </Button>
            </div>

            <div class="flex min-w-0 flex-col gap-5">
                <section class="flex flex-col gap-2.5">
                    <h2 class="m-0 text-base font-semibold text-[var(--color-text-primary)]">
                        {{ formatMessage(messages.background) }}
                    </h2>
                    <div data-tooltip-group="icon-backgrounds" class="flex flex-wrap gap-2">
                        <button
                            v-for="iconBackground in backgrounds"
                            :key="iconBackground.id"
                            v-tooltip="formatMessage(iconBackground.name)"
                            type="button"
                            class="h-10 w-10 cursor-pointer rounded-xl border-2 border-solid transition-transform hover:scale-105 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-brand disabled:cursor-wait disabled:opacity-60"
                            :class="
                                iconBackground.id === selectedBackgroundId
                                    ? 'border-brand shadow-md'
                                    : 'border-transparent'
                            "
                            :style="iconBackground.style"
                            :aria-label="formatMessage(iconBackground.name)"
                            :aria-pressed="iconBackground.id === selectedBackgroundId"
                            :disabled="saving"
                            @click="selectedBackgroundId = iconBackground.id"
                        />
                    </div>
                </section>

                <section
                    v-for="group in iconGroups"
                    :key="group.id"
                    class="flex min-w-0 flex-col gap-2.5"
                >
                    <h2 class="m-0 text-base font-semibold text-[var(--color-text-primary)]">
                        {{ formatMessage(group.name) }}
                    </h2>
                    <div data-tooltip-group="instance-icons" class="grid grid-cols-6 gap-2">
                        <button
                            v-for="icon in group.icons"
                            :key="icon.id"
                            v-tooltip="formatMessage(icon.name)"
                            type="button"
                            class="flex min-w-0 cursor-pointer items-center rounded-xl border border-solid bg-surface-2 p-2 text-[var(--color-text-tertiary)] transition-colors hover:border-brand hover:bg-brand-highlight hover:text-[var(--color-text-primary)] focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-brand disabled:cursor-wait disabled:opacity-60"
                            :class="
                                icon.id === selectedIconId
                                    ? 'border-brand bg-brand-highlight text-[var(--color-text-primary)]'
                                    : 'border-surface-5'
                            "
                            :aria-label="formatMessage(icon.name)"
                            :aria-pressed="icon.id === selectedIconId"
                            :disabled="saving"
                            @click="selectedIconId = icon.id"
                        >
                            <img
                                :src="icon.url"
                                alt=""
                                class="aspect-square w-full object-contain"
                                loading="lazy"
                                decoding="async"
                            />
                        </button>
                    </div>
                </section>
            </div>
        </div>

        <template #actions>
            <div class="flex w-full items-center justify-between gap-2">
                <Button type="outlined" :disabled="saving" @click="selectUploadedIcon"
                    ><UploadIcon />
                    {{ formatMessage(messages.upload) }}
                </Button>
                <Button type="colored" color="brand" :disabled="saving" @click="saveGeneratedIcon"
                    ><SpinnerIcon v-if="saving" class="animate-spin" />
                    <SaveIcon v-else />
                    {{ formatMessage(saving ? messages.saving : messages.useIcon) }}
                </Button>
            </div>
        </template>

        <input
            ref="fileInput"
            type="file"
            accept=".png,.jpeg,.jpg,.svg,.webp,.gif,image/png,image/jpeg,image/svg+xml,image/webp,image/gif"
            class="hidden"
            @change="handleFileInput"
        />
    </PreviewModal>
</template>
