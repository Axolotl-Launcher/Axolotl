<script setup lang="ts">
import CheckIcon from '@modrinth/assets/icons/check.svg?component'
import EditIcon from '@modrinth/assets/icons/edit.svg?component'
import EyeIcon from '@modrinth/assets/icons/eye.svg?component'
import LogInIcon from '@modrinth/assets/icons/log-in.svg?component'
import PlusIcon from '@modrinth/assets/icons/plus.svg?component'
import RotateCounterClockwiseIcon from '@modrinth/assets/icons/rotate-counter-clockwise.svg?component'
import SpinnerIcon from '@modrinth/assets/icons/spinner.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import NavTabs from '@modrinth/ui/src/components/base/NavTabs.vue'
import ArmorPreviewControls from '@modrinth/ui/src/components/skin/ArmorPreviewControls.vue'
import SkinPreviewRenderer from '@modrinth/ui/src/components/skin/SkinPreviewRenderer.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { injectNotificationManager } from '@modrinth/ui/src/providers/web-notifications.ts'
import { commonMessages } from '@modrinth/ui/src/utils/common-messages.ts'
import { computed, inject, onUnmounted, type Ref, ref, useTemplateRef, watch } from 'vue'
import { onBeforeRouteLeave, useRouter } from 'vue-router'

import { usePreview } from '~/components/launcher-preview/lib/state'
import PreviewConfirmModal from '~/components/launcher-preview/overlays/PreviewConfirmModal.vue'

import { bakeSkinPreviews } from './bake'
import { messages } from './messages'
import ArmorTrimTab from './parts/ArmorTrimTab.vue'
import EditSkinModal from './parts/EditSkinModal.vue'
import SkinListSkeleton from './parts/SkinListSkeleton.vue'
import VirtualSkinSectionList from './parts/VirtualSkinSectionList.vue'
import {
    type Cape,
    defaultSectionTitleKey,
    type DefaultSkinSection,
    filterDefaultSkins,
    filterSavedSkins,
    readSkinFile,
    sectionSortIndex,
    type Skin,
    skinsMatch,
    useSkinsState,
} from './state'

/**
 * Port of the app's skin selector (apps/app-frontend/src/pages/Skins.vue).
 *
 * Left column: the player preview with its pending-change actions and the
 * armour toggle. Right column: the saved/official skin tabs, the list itself
 * (sections, drag reordering, add tile, edit and delete overlays) and, behind
 * the armour toggle, the trim panel. Native pieces are simulated in memory:
 * textures are read from disk through the file input, the model is detected
 * from the arm pixels, and applying a skin equips it in the shared accounts
 * state.
 */

const ARMOR_TRIM_PANEL_ID = 'skins-armor-trim-panel'

const { formatMessage } = useVIntl()
const router = useRouter()
const state = usePreview()
const { addNotification, handleError } = injectNotificationManager()

const skinsState = useSkinsState()
const {
    skins,
    originalSelectedSkin,
    selectedSkin,
    armorPreviewConfig,
    savedArmorPreview,
    capes,
    hasPendingSkinChange,
} = skinsState

const editSkinModal = useTemplateRef<InstanceType<typeof EditSkinModal>>('editSkinModal')
const addSkinFileInput = useTemplateRef<HTMLInputElement>('addSkinFileInput')
const skinPreviewArea = useTemplateRef<HTMLElement>('skinPreviewArea')
const deleteSkinModal = useTemplateRef<InstanceType<typeof PreviewConfirmModal>>('deleteSkinModal')

const currentUser = computed(() => state.accounts.selected.value)
const currentAccountType = computed(() => currentUser.value?.account_type)
const username = computed(() => currentUser.value?.profile?.name)
/**
 * The preview is never actually offline; the flag the settings table carries
 * stands in for the network status the app reads at runtime.
 */
const offline = computed(() => state.settings.feature_flags.preview_offline === true)
const isApplyingSkin = ref(false)
const isSkinListBuffering = ref(false)
const isDraggingSkinFile = ref(false)
const isAddSkinButtonDragActive = ref(false)
const skinToDelete = ref<Skin | null>(null)
const skinListTab = ref<'saved' | 'default' | 'armor'>('saved')
const lastSkinListTab = ref<'saved' | 'default'>('saved')
/** The last file the page refused, shown under the list the way the app toasts it. */
const failedFileName = ref<string | null>(null)

let skinListBufferTimer: ReturnType<typeof setTimeout> | undefined
let isUnmounted = false

const isSkinManagementReadOnly = computed(
    () =>
        currentAccountType.value === 'yggdrasil' ||
        (currentAccountType.value !== 'offline' && offline.value),
)

const skinNametag = computed(() => username.value)

const savedSkins = computed(() => {
    try {
        return filterSavedSkins(skins.value)
    } catch (error) {
        handleError(error instanceof Error ? error : new Error(String(error)))
        return []
    }
})

const defaultSkins = computed(() => filterDefaultSkins(skins.value))

const defaultSkinSections = computed<DefaultSkinSection[]>(() => {
    const sections = new Map<string, Skin[]>()

    for (const skin of defaultSkins.value) {
        const section = skin.section ?? 'Default skins'
        const sectionSkins = sections.get(section)
        if (sectionSkins) sectionSkins.push(skin)
        else sections.set(section, [skin])
    }

    return Array.from(sections, ([section, entrySkins]) => ({
        section,
        title: getDefaultSkinSectionTitle(section),
        skins: entrySkins,
    })).sort((a, b) => sectionSortIndex(a.section) - sectionSortIndex(b.section))
})

const currentCape = computed(() => {
    if (selectedSkin.value?.cape_id) {
        return capes.value.find((cape) => cape.id === selectedSkin.value?.cape_id)
    }
    return undefined
})

const skinTexture = computed(() => selectedSkin.value?.texture ?? '')
const capeTexture = computed(() => currentCape.value?.texture)
const skinVariant = computed(() => (selectedSkin.value?.variant === 'SLIM' ? 'SLIM' : 'CLASSIC'))

const skinListTabLinks = computed(() => [
    { href: 'saved', label: formatMessage(messages.savedTab) },
    { href: 'default', label: formatMessage(messages.defaultTab) },
])

watch(skinListTab, (tab, previous) => {
    if (tab === 'armor' || previous === 'armor') return

    isSkinListBuffering.value = true
    clearTimeout(skinListBufferTimer)
    skinListBufferTimer = setTimeout(() => {
        isSkinListBuffering.value = false
    }, 240)
})

watch(
    skins,
    (nextSkins) => {
        void bakeSkinPreviews(nextSkins)
    },
    { immediate: true },
)

watch(isSkinManagementReadOnly, (readOnly) => {
    if (readOnly) {
        isDraggingSkinFile.value = false
        isAddSkinButtonDragActive.value = false
    }
})

onUnmounted(() => {
    isUnmounted = true
    clearTimeout(skinListBufferTimer)
})

function getDefaultSkinSectionTitle(section: string) {
    const key = defaultSectionTitleKey(section)
    // The app shows a section it has no title for (e.g. "Modrinth Pride") by its name.
    return key ? formatMessage(messages[key as keyof typeof messages]) : section
}

function isSkinSelected(skin: Skin) {
    return skinsMatch(selectedSkin.value, skin)
}

function isSkinActive(skin: Skin) {
    return hasPendingSkinChange.value && skinsMatch(originalSelectedSkin.value, skin)
}

function changeSkin(newSkin: Skin) {
    if (isSkinManagementReadOnly.value) return
    skinsState.setSelectedSkin(newSkin)
}

function resetSelectedSkin() {
    skinsState.setSelectedSkin(
        skins.value.find((skin) => skinsMatch(skin, originalSelectedSkin.value)) ??
            originalSelectedSkin.value ??
            skins.value[0],
    )
}

function selectSkinListTab(index: number) {
    if (skinListTab.value === 'armor') resetArmorPreviewToSaved()

    lastSkinListTab.value = index === 0 ? 'saved' : 'default'
    skinListTab.value = lastSkinListTab.value
}

/* ------------------------------------------------------------- armour tab */

function openArmorTab() {
    if (skinListTab.value !== 'armor') lastSkinListTab.value = skinListTab.value
    skinListTab.value = 'armor'
}

function resetArmorPreviewToSaved() {
    skinsState.resetArmorPreviewToSaved()
}

function restoreDefaultArmorPreview() {
    skinsState.restoreDefaultArmorPreview()
}

function closeArmorTab() {
    resetArmorPreviewToSaved()
    skinListTab.value = lastSkinListTab.value

    const focused = document.activeElement
    if (focused instanceof HTMLElement && focused.closest('[data-armor-surface]')) focused.blur()
}

/* --------------------------------------------------------------- applying */

function setLocallyEquippedSkin(skinToApply: Skin) {
    skinsState.equipSkin(skinToApply)
}

async function applySelectedSkin() {
    const skinToApply = selectedSkin.value
    if (
        !skinToApply ||
        !hasPendingSkinChange.value ||
        isApplyingSkin.value ||
        isSkinManagementReadOnly.value
    ) {
        return
    }

    isApplyingSkin.value = true
    try {
        // The app: `equip_skin` posts to Mojang, then the refresh catches up.
        await new Promise((resolve) => setTimeout(resolve, 420))
        if (isUnmounted) return
        setLocallyEquippedSkin(skinToApply)
    } catch (error) {
        handleError(error as Error)
    } finally {
        isApplyingSkin.value = false
    }
}

/* ------------------------------------------------------------ add / edit */

function confirmDeleteSkin(skin: Skin) {
    if (isSkinManagementReadOnly.value) return
    skinToDelete.value = skin
    deleteSkinModal.value?.show()
}

function deleteSkin() {
    if (isSkinManagementReadOnly.value) return
    const deletedSkin = skinToDelete.value
    if (!deletedSkin) return

    try {
        // The app shows nothing on a successful delete; the list simply loses a
        // card and the preview falls back to the equipped skin.
        skinsState.deleteCustomSkin(deletedSkin)
    } finally {
        skinToDelete.value = null
    }
}

function isSkinImagePath(path: string) {
    return path.toLowerCase().endsWith('.png')
}

function isSkinFileDrag(event: DragEvent) {
    const items = Array.from(event.dataTransfer?.items ?? [])
    const files = Array.from(event.dataTransfer?.files ?? [])

    return (
        items.some((item) => item.kind === 'file' && item.type === 'image/png') ||
        files.some((file) => file.type === 'image/png' || isSkinImagePath(file.name))
    )
}

function onAddSkinDragOver(event: DragEvent) {
    if (isSkinManagementReadOnly.value) return
    if (!isSkinFileDrag(event)) return
    event.preventDefault()
    isAddSkinButtonDragActive.value = true
}

function onAddSkinDragLeave() {
    if (isSkinManagementReadOnly.value) return
    isAddSkinButtonDragActive.value = false
}

function onAddSkinDrop(event: DragEvent) {
    if (isSkinManagementReadOnly.value) return
    isAddSkinButtonDragActive.value = false
    isDraggingSkinFile.value = false

    const files = Array.from(event.dataTransfer?.files ?? [])
    const file = files.find(
        (candidate) => candidate.type === 'image/png' || isSkinImagePath(candidate.name),
    )
    if (!file) {
        // A dropped file that is not a PNG is refused with the app's own copy.
        if (files.length > 0) {
            failedFileName.value = files[0].name
            addNotification({
                type: 'error',
                title: formatMessage(messages.droppedFileErrorTitle),
                text: formatMessage(messages.droppedFileErrorText),
            })
        }
        return
    }

    void processSkinFile(file)
}

/** The whole page accepts a dropped PNG as a new skin, as the app does. */
function onPageDragOver(event: DragEvent) {
    if (isSkinManagementReadOnly.value) return
    if (!isSkinFileDrag(event)) return
    event.preventDefault()
    isDraggingSkinFile.value = true
}

function onPageDragLeave(event: DragEvent) {
    if (!isDraggingSkinFile.value) return
    const related = event.relatedTarget
    if (related instanceof Node && document.documentElement.contains(related)) return
    isDraggingSkinFile.value = false
}

let lastDroppedFile: File | null = null
let lastDropAt = 0

function onPageDrop(event: DragEvent) {
    if (isSkinManagementReadOnly.value) return
    isDraggingSkinFile.value = false
    // A drop on the add tile bubbles up here as well; the same file is only
    // handled once.
    const file = Array.from(event.dataTransfer?.files ?? [])[0] ?? null
    if (file && file === lastDroppedFile && Date.now() - lastDropAt < 300) return
    lastDroppedFile = file
    lastDropAt = Date.now()
    onAddSkinDrop(event)
}

function openAddSkinFileBrowser() {
    if (isSkinManagementReadOnly.value) return
    addSkinFileInput.value?.click()
}

async function onAddSkinFileInputChange(event: Event) {
    if (isSkinManagementReadOnly.value) return
    const file = (event.target as HTMLInputElement).files?.[0]
    if (!file) return

    await processSkinFile(file)
    if (addSkinFileInput.value) addSkinFileInput.value.value = ''
}

async function processSkinFile(file: File) {
    if (isSkinManagementReadOnly.value) return

    failedFileName.value = null
    try {
        const texture = await readSkinFile(file)
        const fakeEvent = new MouseEvent('click')
        await editSkinModal.value?.showNew(fakeEvent, texture)
    } catch {
        failedFileName.value = file.name
        addNotification({
            type: 'error',
            title: formatMessage(messages.droppedFileErrorTitle),
            text: formatMessage(messages.droppedFileErrorText),
        })
    }
}

function onSkinSaved(options: { applied: boolean; skin?: Skin; previousSkin?: Skin }) {
    if (!options.skin) return

    const saved = options.skin
    let stored: Skin

    if (options.previousSkin) {
        stored = skinsState.updateCustomSkin(saved, options.previousSkin)
    } else {
        stored = skinsState.addCustomSkin({
            name: `Custom skin ${savedSkins.value.length + 1}`,
            section: undefined,
            variant: saved.variant,
            cape_id: saved.cape_id,
            texture: saved.texture,
            is_equipped: false,
        })
    }

    if (options.applied) {
        void (async () => {
            isApplyingSkin.value = true
            try {
                await new Promise((resolve) => setTimeout(resolve, 420))
                if (isUnmounted) return
                setLocallyEquippedSkin(stored)
            } finally {
                isApplyingSkin.value = false
            }
        })()
    }
}

/* ------------------------------------------------------------ drag order */

function reorderSavedSkins(ordered: Skin[]) {
    skinsState.reorderCustomSkins(ordered)
}

/* ------------------------------------------------------------ navigation */

const removeNavigationFailureHandler = router.afterEach((to, _from, failure) => {
    if (failure && skinPreviewArea.value) {
        skinPreviewArea.value.style.visibility = ''
    }
})

onBeforeRouteLeave(async () => {
    const previewArea = skinPreviewArea.value
    if (!previewArea) return true
    previewArea.style.visibility = 'hidden'
    await new Promise<void>((resolve) => {
        if (typeof window === 'undefined') {
            resolve()
            return
        }
        window.requestAnimationFrame(() => window.requestAnimationFrame(() => resolve()))
    })
    return true
})

onUnmounted(() => {
    removeNavigationFailureHandler()
})

/* --------------------------------------------------------------- sign in */

/** The app signs in through the account card App.vue provides. */
const accountsCard = inject<Ref<{ login: () => void; loginDisabled: boolean } | null>>(
    'accountsCard',
    ref(null),
)

function login() {
    if (offline.value) return
    accountsCard.value?.login()
}

const capeList = computed<Cape[]>(() => capes.value)
</script>

<template>
    <EditSkinModal ref="editSkinModal" :capes="capeList" @saved="onSkinSaved" />
    <input
        ref="addSkinFileInput"
        type="file"
        accept="image/png"
        class="hidden"
        @change="onAddSkinFileInputChange"
    />
    <PreviewConfirmModal
        ref="deleteSkinModal"
        :title="formatMessage(messages.deleteSkinTitle)"
        :description="formatMessage(messages.deleteSkinDescription)"
        :proceed-label="formatMessage(commonMessages.deleteLabel)"
        @proceed="deleteSkin"
    />
    <Teleport
        v-if="currentUser && currentAccountType === 'offline'"
        to="#sidebar-default-teleport-target"
    >
        <section class="p-4">
            <h3 class="m-0 text-base font-semibold text-[var(--color-text-default)]">
                {{ formatMessage(messages.offlineCompatibilityTitle) }}
            </h3>
            <p class="mb-0 mt-2 text-sm leading-6 text-[var(--color-text-tertiary)]">
                {{ formatMessage(messages.offlineCompatibility) }}
            </p>
        </section>
    </Teleport>
    <Teleport
        v-if="currentUser && currentAccountType === 'yggdrasil'"
        to="#sidebar-default-teleport-target"
    >
        <section class="p-4">
            <h3 class="m-0 text-base font-semibold text-[var(--color-text-default)]">
                {{ formatMessage(messages.thirdPartyManagementTitle) }}
            </h3>
            <p class="mb-0 mt-2 text-sm leading-6 text-[var(--color-text-tertiary)]">
                {{ formatMessage(messages.thirdPartyManagementDescription) }}
            </p>
        </section>
    </Teleport>

    <div
        v-if="currentUser"
        data-onboarding-id="skins-page"
        class="skin-layout box-border min-h-full p-4"
        @dragover="onPageDragOver"
        @dragleave="onPageDragLeave"
        @drop.prevent="onPageDrop"
    >
        <div class="sticky top-6 self-start p-2 pt-0">
            <h1 class="m-0 flex items-center gap-2 text-2xl font-bold">
                {{ formatMessage(messages.skinSelectorTitle) }}
            </h1>
            <!--
				The app sizes this with `100vh` less the top bar and the action row;
				inside the launcher window that is the window's own height, so the
				column keeps its height while actions come and go.
			-->
            <div
                ref="skinPreviewArea"
                class="ml-5 mt-4 flex h-[calc(800px_-_var(--top-bar-height)_-_11rem)] min-h-[14rem] items-center justify-center"
            >
                <SkinPreviewRenderer
                    v-model:armor-config="armorPreviewConfig"
                    armor-preview
                    :armor-controls="false"
                    :cape-src="capeTexture"
                    :texture-src="skinTexture || ''"
                    :variant="skinVariant"
                    :nametag="skinNametag"
                    :initial-rotation="Math.PI / 8"
                >
                    <template v-if="hasPendingSkinChange" #nametag-badge>
                        <div
                            class="flex w-max shrink-0 items-center justify-center gap-1.5 whitespace-nowrap rounded-full bg-bg-blue px-3 py-1 text-sm font-semibold leading-5 text-brand-blue"
                        >
                            <EyeIcon class="size-5 shrink-0" />
                            {{ formatMessage(messages.previewingBadge) }}
                        </div>
                    </template>
                </SkinPreviewRenderer>
            </div>

            <div class="ml-5 mt-3 flex flex-wrap items-center justify-center gap-2">
                <template v-if="hasPendingSkinChange">
                    <button
                        class="flex h-10 min-w-0 cursor-pointer items-center justify-center gap-2 rounded-[14px] border-0 bg-surface-4 px-4 py-2.5 text-base font-semibold leading-5 shadow-md transition-[filter,transform] duration-200 enabled:hover:brightness-[--hover-brightness] enabled:focus-visible:brightness-[--hover-brightness] enabled:active:scale-95 disabled:cursor-not-allowed disabled:opacity-50 [&>svg]:size-5 [&>svg]:shrink-0"
                        :disabled="isApplyingSkin || isSkinManagementReadOnly"
                        @click="resetSelectedSkin"
                    >
                        <RotateCounterClockwiseIcon />
                        {{ formatMessage(commonMessages.resetButton) }}
                    </button>
                    <button
                        class="flex h-10 min-w-0 cursor-pointer items-center justify-center gap-2 rounded-[14px] border-0 bg-brand px-4 py-2.5 text-base font-semibold leading-5 text-[rgba(0,0,0,0.9)] shadow-md transition-[filter,transform] duration-200 enabled:hover:brightness-[--hover-brightness] enabled:focus-visible:brightness-[--hover-brightness] enabled:active:scale-95 disabled:cursor-not-allowed disabled:opacity-50 [&>svg]:size-5 [&>svg]:shrink-0"
                        :disabled="isApplyingSkin || isSkinManagementReadOnly"
                        @click="applySelectedSkin"
                    >
                        <SpinnerIcon v-if="isApplyingSkin" class="animate-spin" />
                        <CheckIcon v-else />
                        {{ formatMessage(messages.applyButton) }}
                    </button>
                </template>
                <button
                    v-else
                    class="flex h-10 min-w-0 cursor-pointer items-center justify-center gap-2 rounded-[14px] border-0 bg-surface-4 px-4 py-2.5 text-base font-semibold leading-5 shadow-md transition-[filter,transform] duration-200 enabled:hover:brightness-[--hover-brightness] enabled:focus-visible:brightness-[--hover-brightness] enabled:active:scale-95 disabled:cursor-not-allowed disabled:opacity-50 [&>svg]:size-5 [&>svg]:shrink-0"
                    :disabled="!selectedSkin || isSkinManagementReadOnly"
                    @click="
                        (event: MouseEvent) =>
                            selectedSkin && editSkinModal?.show(event, selectedSkin)
                    "
                >
                    <EditIcon />
                    {{ formatMessage(messages.previewEditButton) }}
                </button>
                <div data-armor-surface class="pointer-events-auto">
                    <ArmorPreviewControls
                        v-model="armorPreviewConfig"
                        panel="external"
                        :panel-id="ARMOR_TRIM_PANEL_ID"
                        :open="skinListTab === 'armor'"
                        @open="openArmorTab"
                        @close="closeArmorTab"
                    />
                </div>
            </div>
        </div>

        <div class="pt-2">
            <div class="mb-4 flex flex-wrap items-center justify-between gap-3">
                <NavTabs
                    :active-index="
                        skinListTab === 'armor' ? -1 : lastSkinListTab === 'saved' ? 0 : 1
                    "
                    :links="skinListTabLinks"
                    mode="local"
                    @tab-click="selectSkinListTab"
                />
                <Button type="colored" color="brand" @click="router.push('/lab/skin-editor')"
                    ><PlusIcon />
                    {{ formatMessage(messages.createSkinButton) }}
                </Button>
            </div>
            <Transition name="armor-tab" mode="out-in">
                <div
                    v-if="skinListTab !== 'armor'"
                    key="list"
                    role="tabpanel"
                    :aria-labelledby="'nav-tab-' + lastSkinListTab"
                >
                    <SkinListSkeleton v-if="isSkinListBuffering" />
                    <div
                        v-if="failedFileName"
                        class="mb-4 flex items-start justify-between gap-3 rounded-xl border border-solid border-red bg-bg-red p-3 text-sm leading-6 text-red"
                        role="alert"
                    >
                        <span>
                            <strong class="font-semibold">{{
                                formatMessage(messages.droppedFileErrorTitle)
                            }}</strong>
                            — {{ formatMessage(messages.droppedFileErrorText) }}
                        </span>
                        <button
                            type="button"
                            class="cursor-pointer border-0 bg-transparent p-0 text-sm font-semibold text-[var(--color-text-primary)] underline"
                            @click="failedFileName = null"
                        >
                            {{ formatMessage(commonMessages.closeButton) }}
                        </button>
                    </div>
                    <VirtualSkinSectionList
                        v-show="!isSkinListBuffering"
                        :active-tab="lastSkinListTab"
                        :saved-skins="savedSkins"
                        :default-skin-sections="defaultSkinSections"
                        :is-skin-selected="isSkinSelected"
                        :is-skin-active="isSkinActive"
                        :is-add-skin-button-drag-active="isAddSkinButtonDragActive"
                        :read-only="isSkinManagementReadOnly"
                        @select="changeSkin"
                        @edit="(skin: Skin, event: MouseEvent) => editSkinModal?.show(event, skin)"
                        @delete="confirmDeleteSkin"
                        @reorder-saved-skins="reorderSavedSkins"
                        @add-skin="openAddSkinFileBrowser"
                        @add-skin-dragenter="onAddSkinDragOver"
                        @add-skin-dragover="onAddSkinDragOver"
                        @add-skin-dragleave="onAddSkinDragLeave"
                        @add-skin-drop="onAddSkinDrop"
                    />
                </div>
                <ArmorTrimTab
                    v-else
                    key="armor"
                    v-model="armorPreviewConfig"
                    :panel-id="ARMOR_TRIM_PANEL_ID"
                    :saved="savedArmorPreview"
                    @save="skinsState.saveArmorPreview"
                    @reset="resetArmorPreviewToSaved"
                    @defaults="restoreDefaultArmorPreview"
                    @close="closeArmorTab"
                />
            </Transition>
        </div>
    </div>

    <div
        v-else
        data-onboarding-id="skins-page"
        class="box-border flex min-h-full items-center justify-center pt-[25%]"
    >
        <div
            class="relative mx-auto flex w-full max-w-xl flex-col gap-5 rounded-lg bg-surface-3 p-7 shadow-lg"
        >
            <img
                src="/axolotl.png"
                :alt="formatMessage(messages.axolotlAlt)"
                class="absolute -top-32 right-8 h-36 w-36 object-contain md:right-20"
            />
            <div
                class="absolute left-0 top-0 h-[1px] w-full bg-gradient-to-r from-transparent via-green-500 to-transparent opacity-40"
                style="
                    background: linear-gradient(
                        to right,
                        transparent 2rem,
                        var(--color-green) calc(100% - 13rem),
                        var(--color-green) calc(100% - 5rem),
                        transparent calc(100% - 2rem)
                    );
                "
            ></div>

            <div class="flex flex-col gap-5">
                <h1 class="m-0 text-3xl font-extrabold">
                    {{ formatMessage(messages.signInTitle) }}
                </h1>
                <p class="m-0 text-lg">
                    {{ formatMessage(messages.signInDescription) }}
                </p>
                <Button
                    v-if="!offline"
                    v-show="accountsCard"
                    type="colored"
                    color="brand"
                    :disabled="accountsCard?.loginDisabled"
                    @click="login"
                    ><LogInIcon v-if="!accountsCard?.loginDisabled" />
                    <SpinnerIcon v-else class="animate-spin" />
                    {{ formatMessage(messages.signInButton) }}
                </Button>
            </div>
        </div>
    </div>
</template>

<style lang="scss" scoped>
.skin-layout {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 2.5fr);
    gap: 2.5rem;

    @media (max-width: 700px) {
        grid-template-columns: 1fr;
    }
}

// The armour tab crossfades with the skin list it replaces.
.armor-tab-enter-active,
.armor-tab-leave-active {
    transition:
        opacity 180ms ease,
        transform 180ms ease;
}

.armor-tab-enter-from,
.armor-tab-leave-to {
    opacity: 0;
    transform: translateY(0.5rem);
}
</style>
