<template>
    <NewModal
        ref="modal"
        :header="formatMessage(messages.title)"
        :on-hide="handleHide"
        width="min(760px, calc(100vw - 2rem))"
        max-width="760px"
        scrollable
    >
        <div class="grid gap-6 md:grid-cols-[220px_minmax(0,1fr)]">
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
                                    ? 'border-contrast shadow-md'
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
                    <div
                        data-tooltip-group="instance-icons"
                        class="grid grid-cols-4 gap-2 sm:grid-cols-6"
                    >
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
    </NewModal>
</template>

<script setup lang="ts">
import { RefreshCwIcon, SaveIcon, SpinnerIcon, UploadIcon } from '@modrinth/assets'
import {
    Button,
    defineMessage,
    defineMessages,
    injectNotificationManager,
    type MessageDescriptor,
    NewModal,
    type PickedFile,
    useVIntl,
} from '@modrinth/ui'
import { convertFileSrc } from '@tauri-apps/api/core'
import { computed, ref, useTemplateRef } from 'vue'

import { cache_icon } from '@/helpers/instance'
import { builtInInstanceIcons, modrinth3DInstanceIcons } from '@/helpers/instance-icons'
import { pickImage } from '@/providers/setup/file-picker'

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

const backgroundNames = defineMessages({
    transparent: {
        id: 'app.instance.icon-picker.background.transparent',
        defaultMessage: 'Transparent',
    },
    grass: { id: 'app.instance.icon-picker.background.grass', defaultMessage: 'Grass' },
    ocean: { id: 'app.instance.icon-picker.background.ocean', defaultMessage: 'Ocean' },
    amethyst: { id: 'app.instance.icon-picker.background.amethyst', defaultMessage: 'Amethyst' },
    sunset: { id: 'app.instance.icon-picker.background.sunset', defaultMessage: 'Sunset' },
    cherry: { id: 'app.instance.icon-picker.background.cherry', defaultMessage: 'Cherry' },
    nether: { id: 'app.instance.icon-picker.background.nether', defaultMessage: 'Nether' },
    slime: { id: 'app.instance.icon-picker.background.slime', defaultMessage: 'Slime' },
    deepDark: { id: 'app.instance.icon-picker.background.deep-dark', defaultMessage: 'Deep Dark' },
    stone: { id: 'app.instance.icon-picker.background.stone', defaultMessage: 'Stone' },
    midnight: { id: 'app.instance.icon-picker.background.midnight', defaultMessage: 'Midnight' },
})

const backgrounds = [
    {
        id: 'transparent',
        name: backgroundNames.transparent,
        colors: null,
        style: {
            backgroundColor: '#ffffff',
            backgroundImage:
                'linear-gradient(45deg, #d7d9de 25%, transparent 25%), linear-gradient(-45deg, #d7d9de 25%, transparent 25%), linear-gradient(45deg, transparent 75%, #d7d9de 75%), linear-gradient(-45deg, transparent 75%, #d7d9de 75%)',
            backgroundPosition: '0 0, 0 8px, 8px -8px, -8px 0',
            backgroundSize: '16px 16px',
        },
    } satisfies IconBackground,
    background('grass', backgroundNames.grass, '#7fc95b', '#2f7d32'),
    background('ocean', backgroundNames.ocean, '#55b7ff', '#3157c8'),
    background('amethyst', backgroundNames.amethyst, '#c084fc', '#6d28d9'),
    background('sunset', backgroundNames.sunset, '#ffba52', '#e94b64'),
    background('cherry', backgroundNames.cherry, '#ff9ec4', '#b93670'),
    background('nether', backgroundNames.nether, '#ef5a46', '#6d1717'),
    background('slime', backgroundNames.slime, '#b6ee55', '#44972f'),
    background('deep-dark', backgroundNames.deepDark, '#245369', '#0c1f2b'),
    background('stone', backgroundNames.stone, '#aeb5bd', '#525b66'),
    background('midnight', backgroundNames.midnight, '#4d5f8f', '#171b2d'),
]

const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()
const modal = useTemplateRef<InstanceType<typeof NewModal>>('modal')
const iconOptions = [...builtInInstanceIcons, ...modrinth3DInstanceIcons]
const iconGroups = [
    {
        id: 'original',
        name: defineMessage({
            id: 'app.instance.icon-picker.group.original',
            defaultMessage: 'Original icons',
        }),
        icons: builtInInstanceIcons,
    },
    {
        id: 'modrinth-3d',
        name: defineMessage({
            id: 'app.instance.icon-picker.group.modrinth-3d',
            defaultMessage: 'Modrinth 3D icons',
        }),
        icons: modrinth3DInstanceIcons,
    },
]

const selectedBackgroundId = ref(backgrounds[0].id)
const selectedIconId = ref(builtInInstanceIcons[0].id)
const saving = ref(false)
let resolveSelection: ((selection: PickedFile | null) => void) | null = null

const selectedBackground = computed(
    () => backgrounds.find((item) => item.id === selectedBackgroundId.value) ?? backgrounds[0],
)
const selectedIcon = computed(
    () => iconOptions.find((icon) => icon.id === selectedIconId.value) ?? builtInInstanceIcons[0],
)

const messages = defineMessages({
    title: {
        id: 'app.instance.icon-picker.title',
        defaultMessage: 'Create an instance icon',
    },
    description: {
        id: 'app.instance.icon-picker.description',
        defaultMessage: 'Combine a background and a Minecraft element, or upload your own image.',
    },
    background: {
        id: 'app.instance.icon-picker.background',
        defaultMessage: 'Background',
    },
    surpriseMe: {
        id: 'app.instance.icon-picker.surprise-me',
        defaultMessage: 'Surprise me',
    },
    upload: {
        id: 'app.instance.icon-picker.upload',
        defaultMessage: 'Upload image',
    },
    useIcon: {
        id: 'app.instance.icon-picker.use-icon',
        defaultMessage: 'Use this icon',
    },
    saving: {
        id: 'app.instance.icon-picker.saving',
        defaultMessage: 'Saving...',
    },
    loadError: {
        id: 'app.instance.icon-picker.load-error',
        defaultMessage: 'Failed to load the bundled icon.',
    },
})

function finish(selection: PickedFile | null) {
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
    if (!selectedBackground.value.colors) {
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

    if (selectedBackground.value.colors) {
        const gradient = context.createLinearGradient(0, 0, size, size)
        gradient.addColorStop(0, selectedBackground.value.colors[0])
        gradient.addColorStop(1, selectedBackground.value.colors[1])
        context.fillStyle = gradient
        context.fillRect(0, 0, size, size)
    }

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
        const blob = await renderGeneratedIcon()
        const fileName = `generated-${selectedBackgroundId.value}-${selectedIconId.value}.png`
        const bytes = Array.from(new Uint8Array(await blob.arrayBuffer()))
        const path = await cache_icon(fileName, bytes)
        finish({
            file: new File([blob], fileName, { type: 'image/png' }),
            path,
            previewUrl: convertFileSrc(path),
            frameless: selectedBackgroundId.value === 'transparent',
        })
    } catch (error) {
        handleError(error)
    } finally {
        saving.value = false
    }
}

async function selectUploadedIcon() {
    try {
        const selection = await pickImage()
        if (selection) finish(selection)
    } catch (error) {
        handleError(error)
    }
}

function show(): Promise<PickedFile | null> {
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
