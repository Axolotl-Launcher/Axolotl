<script setup lang="ts">
import CheckIcon from '@modrinth/assets/icons/check.svg?component'
import SaveIcon from '@modrinth/assets/icons/save.svg?component'
import SpinnerIcon from '@modrinth/assets/icons/spinner.svg?component'
import UploadIcon from '@modrinth/assets/icons/upload.svg?component'
import XIcon from '@modrinth/assets/icons/x.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import RadioButtons from '@modrinth/ui/src/components/base/RadioButtons.vue'
import CapeLikeTextButton from '@modrinth/ui/src/components/skin/CapeLikeTextButton.vue'
import SkinPreviewRenderer from '@modrinth/ui/src/components/skin/SkinPreviewRenderer.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { useScrollIndicator } from '@modrinth/ui/src/composables/scroll-indicator.ts'
import { commonMessages } from '@modrinth/ui/src/utils/common-messages.ts'
import { computed, nextTick, ref, useTemplateRef, watch } from 'vue'

import PreviewModal from '~/components/launcher-preview/overlays/PreviewModal.vue'

import { messages } from '../messages'
import {
    type Cape,
    determineModelType,
    loadImage,
    normalizeSkinTexture,
    readSkinFile,
    type Skin,
    type SkinModel,
} from '../state'
import CapeThumbnail from './CapeThumbnail.vue'

/**
 * Port of the app's `EditSkinModal` (apps/app-frontend/src/components/ui/skin).
 * A skin is added from an uploaded PNG or edited in place; the model is guessed
 * from the texture with the app's arm-pixel rule and can be overridden, and a
 * cape can be attached. Saving hands the result back to the page, which stores
 * it and re-equips it when it was already equipped.
 */

const CAPE_LIST_MAX_HEIGHT = 334

const props = defineProps<{
    capes?: Cape[]
}>()

const emit = defineEmits<{
    saved: [options: { applied: boolean; skin?: Skin; previousSkin?: Skin }]
}>()

const { formatMessage } = useVIntl()

const modal = useTemplateRef<InstanceType<typeof PreviewModal>>('modal')
const textureFileInput = useTemplateRef<HTMLInputElement>('textureFileInput')
const capeListRef = ref<HTMLElement | null>(null)
const capeListMaxHeight = ref(`${CAPE_LIST_MAX_HEIGHT}px`)
const mode = ref<'new' | 'edit'>('new')
const currentSkin = ref<Skin | null>(null)
const isSaving = ref(false)

const uploadedTextureUrl = ref<{ original: string; normalized: string } | null>(null)
const previewSkin = ref('')
const variant = ref<SkinModel>('CLASSIC')
const selectedCape = ref<Cape | undefined>(undefined)

const selectedCapeTexture = computed(() => selectedCape.value?.texture)
const canEditTextureAndModel = computed(() => currentSkin.value?.source !== 'default')

const {
    showTopFade: showCapeTopFade,
    showBottomFade: showCapeBottomFade,
    checkScrollState: checkCapeScrollState,
    forceCheck: forceCapeScrollCheck,
} = useScrollIndicator(capeListRef)

function updateCapeListLayout() {
    const capeList = capeListRef.value
    const modalContent = capeList?.closest('[data-modal-content]') as HTMLElement | null

    if (!capeList || !modalContent) {
        capeListMaxHeight.value = `${CAPE_LIST_MAX_HEIGHT}px`
        forceCapeScrollCheck()
        return
    }

    const availableHeight =
        modalContent.getBoundingClientRect().bottom - capeList.getBoundingClientRect().top

    capeListMaxHeight.value = `${Math.min(
        CAPE_LIST_MAX_HEIGHT,
        Math.max(0, Math.floor(availableHeight)),
    )}px`

    nextTick(() => forceCapeScrollCheck())
}

let capeListLayoutFrame: number | null = null
function refreshCapeListLayout() {
    if (capeListLayoutFrame !== null) cancelAnimationFrame(capeListLayoutFrame)
    capeListLayoutFrame = requestAnimationFrame(() => {
        capeListLayoutFrame = null
        updateCapeListLayout()
    })
}

const sortedCapes = computed(() =>
    [...(props.capes ?? [])].sort((a, b) =>
        (a.name || '').toLowerCase().localeCompare((b.name || '').toLowerCase()),
    ),
)

async function loadPreviewSkin() {
    if (uploadedTextureUrl.value) {
        previewSkin.value = uploadedTextureUrl.value.normalized
    } else if (currentSkin.value) {
        try {
            previewSkin.value = await normalizeSkinTexture(currentSkin.value.texture, true)
        } catch {
            previewSkin.value = '/launcher-preview/skins/steve.png'
        }
    } else {
        previewSkin.value = '/launcher-preview/skins/steve.png'
    }
}

const hasEdits = computed(() => {
    if (mode.value !== 'edit') return true
    if (uploadedTextureUrl.value) return true
    if (!currentSkin.value) return false
    if (variant.value !== currentSkin.value.variant) return true
    if ((selectedCape.value?.id || null) !== (currentSkin.value.cape_id || null)) return true
    return false
})

const disableSave = computed(
    () =>
        (mode.value === 'new' && !uploadedTextureUrl.value) ||
        (mode.value === 'edit' && !hasEdits.value),
)

const saveTooltip = computed(() => {
    if (isSaving.value) return formatMessage(messages.savingTooltip)
    if (mode.value === 'new' && !uploadedTextureUrl.value) {
        return formatMessage(messages.uploadSkinFirstTooltip)
    }
    if (mode.value === 'edit' && !hasEdits.value) {
        return formatMessage(messages.makeEditFirstTooltip)
    }
    return undefined
})

const isOpen = ref(false)

function resetState() {
    mode.value = 'new'
    currentSkin.value = null
    uploadedTextureUrl.value = null
    previewSkin.value = ''
    variant.value = 'CLASSIC'
    selectedCape.value = undefined
    isSaving.value = false
}

function handleModalHide() {
    isOpen.value = false
    setTimeout(() => resetState(), 250)
}

async function show(e: MouseEvent, skin?: Skin) {
    mode.value = skin ? 'edit' : 'new'
    currentSkin.value = skin ?? null
    if (skin) {
        variant.value = skin.variant
        selectedCape.value = props.capes?.find((cape) => cape.id === skin.cape_id)
    } else {
        variant.value = 'CLASSIC'
        selectedCape.value = undefined
    }

    await loadPreviewSkin()
    isOpen.value = true
    modal.value?.show(e)
    nextTick(() => refreshCapeListLayout())
}

async function showNew(e: MouseEvent, texture: string) {
    const original = texture
    const normalized = await normalizeSkinTexture(texture, true)
    mode.value = 'new'
    currentSkin.value = null
    uploadedTextureUrl.value = { original, normalized }
    variant.value = await determineModelType(original)
    selectedCape.value = undefined

    await loadPreviewSkin()
    isOpen.value = true
    modal.value?.show(e)
    nextTick(() => refreshCapeListLayout())
}

function hide() {
    modal.value?.hide()
}

function selectCape(cape: Cape | undefined) {
    selectedCape.value = cape
}

function openTextureFileBrowser() {
    textureFileInput.value?.click()
}

async function onTextureFileInputChange(e: Event) {
    const files = (e.target as HTMLInputElement).files
    const file = files?.[0]
    if (!file) return

    try {
        const original = await readSkinFile(file)
        const normalized = await normalizeSkinTexture(original, true)
        uploadedTextureUrl.value = { original, normalized }
        await loadPreviewSkin()
        nextTick(() => refreshCapeListLayout())
    } finally {
        if (textureFileInput.value) textureFileInput.value.value = ''
    }
}

async function save() {
    isSaving.value = true
    try {
        const textureUrl = uploadedTextureUrl.value?.original ?? currentSkin.value?.texture ?? ''

        if (mode.value === 'new') {
            const addedSkin: Skin = {
                texture_key: '',
                variant: variant.value === 'SLIM' ? 'SLIM' : 'CLASSIC',
                cape_id: selectedCape.value?.id,
                texture: textureUrl,
                source: 'custom',
                is_equipped: false,
            }
            hide()
            emit('saved', { applied: false, skin: addedSkin })
            return
        }

        const updatedSkin: Skin = {
            ...currentSkin.value!,
            texture: textureUrl,
            variant: variant.value === 'SLIM' ? 'SLIM' : 'CLASSIC',
            cape_id: selectedCape.value?.id,
        }

        const applied = !!currentSkin.value?.is_equipped
        const previousSkin = currentSkin.value!
        hide()
        emit('saved', { applied, skin: updatedSkin, previousSkin })
    } finally {
        isSaving.value = false
    }
}

watch([uploadedTextureUrl, currentSkin], async () => {
    await loadPreviewSkin()
    refreshCapeListLayout()
})

watch(
    capeListRef,
    (capeList, _, onCleanup) => {
        if (!capeList) return
        const modalContent = capeList.closest('[data-modal-content]')
        const resizeObserver = new ResizeObserver(() => refreshCapeListLayout())
        if (modalContent instanceof HTMLElement) resizeObserver.observe(modalContent)
        window.addEventListener('resize', refreshCapeListLayout, { passive: true })
        refreshCapeListLayout()
        onCleanup(() => {
            resizeObserver.disconnect()
            window.removeEventListener('resize', refreshCapeListLayout)
            if (capeListLayoutFrame !== null) {
                cancelAnimationFrame(capeListLayoutFrame)
                capeListLayoutFrame = null
            }
        })
    },
    { flush: 'post' },
)

// A texture can be dropped straight onto the preview while the modal is open.
const isTextureDragActive = ref(false)

function onPreviewDragOver(event: DragEvent) {
    if (!canEditTextureAndModel.value) return
    const file = Array.from(event.dataTransfer?.items ?? []).find(
        (item) => item.kind === 'file' && item.type === 'image/png',
    )
    if (!file) return
    event.preventDefault()
    isTextureDragActive.value = true
}

async function onPreviewDrop(event: DragEvent) {
    if (!canEditTextureAndModel.value) return
    isTextureDragActive.value = false
    const file = Array.from(event.dataTransfer?.files ?? []).find(
        (candidate) => candidate.type === 'image/png',
    )
    if (!file) return
    event.preventDefault()
    try {
        const original = await readSkinFile(file)
        const normalized = await normalizeSkinTexture(original, true)
        uploadedTextureUrl.value = { original, normalized }
        await loadPreviewSkin()
    } catch {
        // A file that is not an image cannot be previewed; the input's own path
        // reports the failure for the add flow.
    }
}

void loadImage

defineExpose({ show, showNew, hide })
</script>

<template>
    <PreviewModal ref="modal" :on-hide="handleModalHide">
        <template #title>
            <span class="text-lg font-extrabold text-[var(--color-text-primary)]">
                {{
                    formatMessage(mode === 'edit' ? messages.editSkinTitle : messages.addSkinTitle)
                }}
            </span>
        </template>

        <div class="flex flex-col gap-6 md:flex-row">
            <div
                class="h-[25rem] w-[16rem] min-w-[16rem] flex-shrink-0 md:self-center"
                :class="{
                    'rounded-2xl outline-dashed outline-2 outline-brand': isTextureDragActive,
                }"
                @dragover="onPreviewDragOver"
                @dragleave="isTextureDragActive = false"
                @drop="onPreviewDrop"
            >
                <SkinPreviewRenderer
                    :variant="variant"
                    :texture-src="previewSkin || ''"
                    :cape-src="selectedCapeTexture"
                    framing="modal"
                    :initial-rotation="Math.PI / 8"
                    class="h-full w-full"
                />
            </div>

            <div class="flex min-h-[20rem] w-full flex-col gap-4">
                <section v-if="mode === 'edit' && canEditTextureAndModel">
                    <h2 class="mb-2 text-base font-semibold">
                        {{ formatMessage(messages.textureSection) }}
                    </h2>
                    <Button class="!shadow-none" @click="openTextureFileBrowser"
                        ><UploadIcon /> {{ formatMessage(messages.replaceTextureButton) }}
                    </Button>
                    <input
                        ref="textureFileInput"
                        type="file"
                        accept="image/png"
                        class="hidden"
                        @change="onTextureFileInputChange"
                    />
                </section>

                <section v-if="canEditTextureAndModel">
                    <h2 class="mb-2 text-base font-semibold">
                        {{ formatMessage(messages.armStyleSection) }}
                    </h2>
                    <RadioButtons v-model="variant" :items="['CLASSIC', 'SLIM']">
                        <template #default="{ item }">
                            {{
                                formatMessage(
                                    item === 'CLASSIC'
                                        ? messages.wideArmStyle
                                        : messages.slimArmStyle,
                                )
                            }}
                        </template>
                    </RadioButtons>
                </section>

                <section>
                    <h2 class="mb-2 text-base font-semibold">
                        {{ formatMessage(messages.capeSection) }}
                    </h2>
                    <div class="relative w-fit max-w-full">
                        <Transition
                            enter-active-class="transition-all duration-200 ease-out"
                            enter-from-class="opacity-0 max-h-0"
                            enter-to-class="opacity-100 max-h-6"
                            leave-active-class="transition-all duration-200 ease-in"
                            leave-from-class="opacity-100 max-h-6"
                            leave-to-class="opacity-0 max-h-0"
                        >
                            <div
                                v-if="showCapeTopFade"
                                class="pointer-events-none absolute left-0 right-0 top-0 z-10 h-6 bg-gradient-to-b from-bg-raised to-transparent"
                            />
                        </Transition>

                        <div
                            ref="capeListRef"
                            class="grid auto-rows-max grid-cols-[repeat(4,max-content)] gap-2 overflow-y-auto pr-1"
                            :style="{ maxHeight: capeListMaxHeight }"
                            @scroll="checkCapeScrollState"
                        >
                            <CapeLikeTextButton
                                :tooltip="formatMessage(messages.noCapeTooltip)"
                                :highlighted="!selectedCape"
                                @click="selectCape(undefined)"
                            >
                                <template #icon><XIcon /></template>
                                <span>{{ formatMessage(messages.noneCapeOption) }}</span>
                            </CapeLikeTextButton>

                            <CapeThumbnail
                                v-for="cape in sortedCapes"
                                :id="cape.id"
                                :key="cape.id"
                                :texture="cape.texture"
                                :name="cape.name || formatMessage(messages.capeFallbackName)"
                                :selected="selectedCape?.id === cape.id"
                                @select="selectCape(cape)"
                            />
                        </div>

                        <Transition
                            enter-active-class="transition-all duration-200 ease-out"
                            enter-from-class="opacity-0 max-h-0"
                            enter-to-class="opacity-100 max-h-6"
                            leave-active-class="transition-all duration-200 ease-in"
                            leave-from-class="opacity-100 max-h-6"
                            leave-to-class="opacity-0 max-h-0"
                        >
                            <div
                                v-if="showCapeBottomFade"
                                class="pointer-events-none absolute bottom-0 left-0 right-0 z-10 h-6 bg-gradient-to-t from-bg-raised to-transparent"
                            />
                        </Transition>
                    </div>
                </section>
            </div>
        </div>

        <template #actions>
            <div class="flex justify-end gap-2">
                <Button type="outlined" :disabled="isSaving" @click="hide"
                    ><XIcon />{{ formatMessage(commonMessages.cancelButton) }}
                </Button>
                <Button
                    v-tooltip="saveTooltip"
                    type="colored"
                    color="brand"
                    :disabled="disableSave || isSaving"
                    @click="save"
                    ><SpinnerIcon v-if="isSaving" class="animate-spin" />
                    <CheckIcon v-else-if="mode === 'new'" />
                    <SaveIcon v-else />
                    {{
                        formatMessage(
                            mode === 'new' ? messages.addSkinModalButton : messages.saveSkinButton,
                        )
                    }}
                </Button>
            </div>
        </template>
    </PreviewModal>
</template>
