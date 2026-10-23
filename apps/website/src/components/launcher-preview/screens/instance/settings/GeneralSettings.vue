<script setup lang="ts">
import CopyIcon from '@modrinth/assets/icons/copy.svg?component'
import EditIcon from '@modrinth/assets/icons/edit.svg?component'
import MonitorIcon from '@modrinth/assets/icons/monitor.svg?component'
import SpinnerIcon from '@modrinth/assets/icons/spinner.svg?component'
import TrashIcon from '@modrinth/assets/icons/trash.svg?component'
import UploadIcon from '@modrinth/assets/icons/upload.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import Chips from '@modrinth/ui/src/components/base/Chips.vue'
import OverflowMenu from '@modrinth/ui/src/components/base/OverflowMenu.vue'
import RadioButtons from '@modrinth/ui/src/components/base/RadioButtons.vue'
import StyledInput from '@modrinth/ui/src/components/base/StyledInput.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { injectNotificationManager } from '@modrinth/ui/src/providers/web-notifications.ts'
import { commonMessages } from '@modrinth/ui/src/utils/common-messages.ts'
import { computed, type Ref, ref, watch } from 'vue'
import { useRouter } from 'vue-router'

import { useInstanceApi } from '../../../lib/platform/instance'
import { createShortcutNotice } from '../../../lib/platform/opener'
import { usePreview } from '../../../lib/state'
import type { GameInstance } from '../../../lib/types'
import { injectPreviewWindow } from '../../../lib/window'
import InstanceIcon from '../../../shell/InstanceIcon.vue'
import { useLocalJobs } from '../../downloads/local-jobs'
import { generalSettingsMessages as messages } from '../messages'
import ConfirmDeleteInstanceModal from './ConfirmDeleteInstanceModal.vue'
import { injectInstanceSettings } from './context'
import InstanceIconPickerModal from './InstanceIconPickerModal.vue'
import { useInstanceSettingsState, useSimulatedInvoke } from './state'
import { INSTANCE_NAME_MAX_LENGTH, useInstanceName } from './use-instance-name'

/**
 * Port of `apps/app-frontend/src/components/ui/instance_settings/GeneralSettings.vue`.
 * `edit`, `edit_icon` and `remove` act on the preview's instance list;
 * `install_duplicate_instance` queues the same duplicate job the Library's
 * context menu does (a copy with the same name, version, loader, icon and link,
 * whose files are copied from the original).
 */
const { handleError } = injectNotificationManager()
const { formatMessage } = useVIntl()
const router = useRouter()
const preview = usePreview()
const { remove } = useInstanceApi()
const localJobs = useLocalJobs()
const settingsState = useInstanceSettingsState()
const simulate = useSimulatedInvoke()
const { layer } = injectPreviewWindow()

const deleteConfirmModal = ref<InstanceType<typeof ConfirmDeleteInstanceModal>>()
const iconPicker = ref<InstanceType<typeof InstanceIconPickerModal>>()

const { instance } = injectInstanceSettings()
type ReleaseChannel = GameInstance['update_channel']
const releaseChannelOptions: ReleaseChannel[] = ['release', 'beta', 'alpha']

const instanceId = instance.value.id
const {
    title,
    saving: savingName,
    failed: nameSaveFailed,
    invalid: nameInvalid,
    saveName,
    normalize: normalizeName,
} = useInstanceName(instanceId, instance.value.name, (name) => edit(instanceId, { name }))
const icon: Ref<string | undefined> = ref(instance.value.icon_path)
const savingReleaseChannel = ref(false)
const selectedReleaseChannel = ref<ReleaseChannel>(instance.value.update_channel)
const releaseChannelDisabledItems = computed<ReleaseChannel[]>(() =>
    savingReleaseChannel.value ? [...releaseChannelOptions] : [],
)

const installing = computed(() => instance.value.install_stage !== 'installed')

/** `edit(instanceId, patch)`. */
async function edit(instanceId: string, patch: Partial<GameInstance>) {
    await simulate()
    const target = preview.instances.get(instanceId)
    if (!target) throw new Error('Unknown instance')
    if (patch.name !== undefined) preview.instances.rename(instanceId, patch.name)
    if (patch.update_channel !== undefined) target.update_channel = patch.update_channel
    target.modified = new Date().toISOString()
}

/** `edit_icon(instanceId, path)`. */
async function editIcon(instanceId: string, path: string | null) {
    await simulate()
    const target = preview.instances.get(instanceId)
    if (!target) throw new Error('Unknown instance')
    if (path) preview.instances.setIcon(instanceId, path)
    else target.icon_path = undefined
}

async function duplicateInstance() {
    try {
        // The copy is made from the instance's files on disk.
        await preview.downloads.ensureContents()
        await simulate(120)
        const source = instance.value
        const job = localJobs.duplicate(source)
        const copyId = job.instance_id
        if (copyId) {
            const contents = preview.downloads.contents.value
            preview.downloads.contents.value = {
                ...contents,
                [copyId]: (contents[source.id] ?? []).map((item) => ({ ...item })),
            }
        }
    } catch (error) {
        handleError(error)
    }
}

const creatingDesktopShortcut = ref(false)
function createDesktopShortcut() {
    if (creatingDesktopShortcut.value) return

    // `createInstanceShortcutOnDesktop` writes a file to ~/Desktop, which only the desktop app can.
    createShortcutNotice()
}

function formatReleaseChannelLabel(channel: ReleaseChannel) {
    switch (channel) {
        case 'release':
            return formatMessage(messages.updateChannelRelease)
        case 'beta':
            return formatMessage(messages.updateChannelBeta)
        case 'alpha':
            return formatMessage(messages.updateChannelAlpha)
    }
}

function formatReleaseChannelDescription(channel: ReleaseChannel) {
    switch (channel) {
        case 'release':
            return formatMessage(messages.updateChannelReleaseDescription)
        case 'beta':
            return formatMessage(messages.updateChannelBetaDescription)
        case 'alpha':
            return formatMessage(messages.updateChannelAlphaDescription)
    }
}

watch(
    () => [instance.value.id, instance.value.update_channel] as const,
    () => {
        if (!savingReleaseChannel.value) {
            selectedReleaseChannel.value = instance.value.update_channel
        }
    },
)

watch(selectedReleaseChannel, async (channel, previousChannel) => {
    const previousReleaseChannel = previousChannel ?? instance.value.update_channel
    if (channel === instance.value.update_channel) return

    savingReleaseChannel.value = true
    const instanceId = instance.value.id
    await edit(instanceId, { update_channel: channel }).catch((error) => {
        selectedReleaseChannel.value = previousReleaseChannel
        handleError(error)
    })
    savingReleaseChannel.value = false
})

async function resetIcon() {
    icon.value = undefined
    await editIcon(instance.value.id, null).catch(handleError)
}

async function setIcon() {
    try {
        const picked = await iconPicker.value?.show()
        if (!picked?.path) return

        const previousIcon = icon.value
        icon.value = picked.path
        try {
            await editIcon(instance.value.id, picked.path)
        } catch (error) {
            icon.value = previousIcon
            handleError(error)
        }
    } catch (error) {
        handleError(error)
    }
}

/* Game directory: the preview's instances use the managed folder unless an import set one. */

function dirname(path: string) {
    const trimmed = path.replace(/\/+$/, '')
    const index = trimmed.lastIndexOf('/')
    return index > 0 ? trimmed.slice(0, index) : '/'
}

function basename(path: string) {
    const trimmed = path.replace(/\/+$/, '')
    return trimmed.slice(trimmed.lastIndexOf('/') + 1)
}

function join(...parts: string[]) {
    return parts.join('/').replace(/\/{2,}/g, '/')
}

const extras = settingsState.extrasFor(instance.value.id)
const gameDirOverride = ref(extras.game_dir_override)
const savingGameDir = ref(false)
const isDirectLinked = computed(() => !!extras.linked_dot_minecraft)
const configuredExternalGameDir = computed(
    () => gameDirOverride.value ?? extras.linked_dot_minecraft ?? null,
)
const externalGameDir = computed(() => configuredExternalGameDir.value)

// An external game dir is stored as a single path. Whether it is version
// isolated is encoded in the path: `<root>/versions/<name>` vs the `.minecraft`
// root itself. `isExternal` is false for built-in (managed) instances, which
// expose no isolation option.
const isExternal = computed(() => !!externalGameDir.value)

const gameDirInfo = computed<{ isolated: boolean; baseRoot: string | null }>(() => {
    const path = configuredExternalGameDir.value
    if (!path || isDirectLinked.value) return { isolated: false, baseRoot: null }
    const parent = dirname(path)
    if (basename(parent).toLowerCase() === 'versions') {
        return { isolated: true, baseRoot: dirname(parent) }
    }
    return { isolated: false, baseRoot: path }
})

type GameDirMode = 'isolated' | 'not-isolated'
const gameDirMode = computed<GameDirMode>({
    get: () => (gameDirInfo.value.isolated ? 'isolated' : 'not-isolated'),
    set: (mode) => void setGameDirMode(mode),
})
const gameDirModeItems: GameDirMode[] = ['isolated', 'not-isolated']

function gameDirModeLabel(mode: GameDirMode) {
    return mode === 'isolated' ? messages.gameDirIsolated : messages.gameDirNotIsolated
}

async function setGameDirMode(mode: GameDirMode) {
    const baseRoot = gameDirInfo.value.baseRoot
    if (!baseRoot) return
    const nextPath =
        mode === 'isolated' ? join(baseRoot, 'versions', instance.value.name) : baseRoot
    if (nextPath === gameDirOverride.value) return

    // The launcher only records the new override path; the user is responsible
    // for actually moving the mods/saves/config folders to match.
    const previous = gameDirOverride.value
    gameDirOverride.value = nextPath
    savingGameDir.value = true
    try {
        await simulate()
        extras.game_dir_override = nextPath
    } catch (error) {
        gameDirOverride.value = previous
        handleError(error)
    } finally {
        savingGameDir.value = false
    }
}

const removing = ref(false)
async function removeInstance() {
    if (removing.value) return
    removing.value = true
    const path = instance.value.id

    try {
        await saveName()
        await remove(path)
        await router.push({ path: '/' })
    } finally {
        removing.value = false
    }
}
</script>

<template>
    <ConfirmDeleteInstanceModal
        ref="deleteConfirmModal"
        :instance-id="instance.id"
        :symlink-target="instance.symlink_target"
        :instances="[instance]"
        :delete-action="removeInstance"
    />
    <InstanceIconPickerModal ref="iconPicker" />
    <div class="block">
        <div class="group relative float-end ml-10 w-fit">
            <div class="flex flex-col gap-1">
                <span class="text-lg font-semibold text-[var(--color-text-primary)]">
                    {{ formatMessage(messages.icon) }}
                </span>
                <div class="group relative w-fit">
                    <!--
                        The app binds `v-tooltip="formatMessage(messages.editIcon)"` to this
                        menu, whose root renders no element, so Vue drops the directive and no
                        tooltip shows; the port leaves the inert directive out.
                    -->
                    <OverflowMenu
                        class="m-0 cursor-pointer appearance-none border-none bg-transparent p-0 transition-transform group-active:scale-95"
                        :container="layer ?? undefined"
                        :options="[
                            {
                                id: 'select',
                                action: () => setIcon(),
                            },
                            {
                                id: 'remove',
                                color: 'danger',
                                action: () => resetIcon(),
                                shown: !!icon,
                            },
                        ]"
                    >
                        <InstanceIcon
                            :icon-path="icon"
                            :instance-id="instance.id"
                            :loader="instance.loader"
                            size="108px"
                            class="transition-[filter] group-hover:brightness-75"
                            no-shadow
                        />
                        <div
                            class="absolute top-0 flex h-full w-full items-center justify-center opacity-0 transition-all group-hover:opacity-100"
                        >
                            <EditIcon
                                aria-hidden="true"
                                class="h-10 w-10 text-[var(--color-text-default)]"
                            />
                        </div>
                        <template #select>
                            <UploadIcon />
                            {{
                                icon
                                    ? formatMessage(messages.replaceIcon)
                                    : formatMessage(messages.selectIcon)
                            }}
                        </template>
                        <template #remove>
                            <TrashIcon /> {{ formatMessage(messages.removeIcon) }}
                        </template>
                    </OverflowMenu>
                </div>
            </div>
        </div>
        <label
            for="instance-name"
            class="m-0 block text-lg font-semibold text-[var(--color-text-primary)]"
        >
            {{ formatMessage(messages.name) }}
        </label>
        <div class="flex">
            <StyledInput
                id="instance-name"
                v-model="title"
                autocomplete="off"
                :maxlength="INSTANCE_NAME_MAX_LENGTH"
                :disabled="removing"
                :error="nameInvalid || nameSaveFailed"
                :aria-describedby="
                    nameInvalid || nameSaveFailed ? 'instance-name-error' : undefined
                "
                wrapper-class="flex-grow"
                @blur="normalizeName"
            />
        </div>
        <p
            v-if="nameInvalid || nameSaveFailed"
            id="instance-name-error"
            class="mb-0 mt-2 text-sm text-red"
        >
            {{ formatMessage(nameInvalid ? messages.nameRequired : messages.nameSaveFailed) }}
            <Button
                v-if="nameSaveFailed && !nameInvalid"
                type="quiet"
                size="2xs"
                :loading="savingName"
                @click="saveName"
            >
                {{ formatMessage(commonMessages.retryButton) }}
            </Button>
        </p>
        <p
            v-else-if="savingName"
            class="mb-0 mt-2 text-sm text-[var(--color-text-tertiary)]"
            role="status"
        >
            {{ formatMessage(commonMessages.savingButton) }}
        </p>
        <template v-if="instance.install_stage == 'installed'">
            <div class="mt-6 flex flex-col gap-2.5">
                <h2
                    id="duplicate-instance-label"
                    class="m-0 block text-lg font-semibold text-[var(--color-text-primary)]"
                >
                    {{ formatMessage(messages.duplicateInstance) }}
                </h2>
                <Button
                    v-tooltip="
                        installing ? formatMessage(messages.duplicateButtonTooltipInstalling) : null
                    "
                    aria-labelledby="duplicate-instance-label"
                    :disabled="installing"
                    class="w-max !shadow-none"
                    @click="duplicateInstance"
                    ><CopyIcon /> {{ formatMessage(messages.duplicateButton) }}
                </Button>
                <p class="m-0">
                    {{ formatMessage(messages.duplicateInstanceDescription) }}
                </p>
            </div>
        </template>
        <div class="mt-6 flex flex-col gap-2.5">
            <h2
                id="desktop-shortcut-label"
                class="m-0 block text-lg font-semibold text-[var(--color-text-primary)]"
            >
                {{ formatMessage(messages.desktopShortcut) }}
            </h2>
            <Button
                aria-labelledby="desktop-shortcut-label"
                :disabled="creatingDesktopShortcut"
                class="w-max !shadow-none"
                @click="createDesktopShortcut"
                ><SpinnerIcon v-if="creatingDesktopShortcut" class="animate-spin" />
                <MonitorIcon v-else aria-hidden="true" />
                {{
                    creatingDesktopShortcut
                        ? formatMessage(messages.creatingDesktopShortcut)
                        : formatMessage(messages.createDesktopShortcut)
                }}
            </Button>
            <p class="m-0">
                {{ formatMessage(messages.desktopShortcutDescription) }}
            </p>
        </div>
        <div class="mt-6 flex flex-col gap-2.5">
            <h2 class="m-0 block text-lg font-semibold text-[var(--color-text-primary)]">
                {{ formatMessage(messages.gameDir) }}
            </h2>
            <p class="m-0">
                {{ formatMessage(messages.gameDirDescription) }}
            </p>
            <template v-if="isExternal">
                <div v-if="!isDirectLinked" class="flex flex-col gap-1.5">
                    <RadioButtons v-model="gameDirMode" :items="gameDirModeItems" force-selection>
                        <template #default="{ item }">
                            {{ formatMessage(gameDirModeLabel(item)) }}
                        </template>
                    </RadioButtons>
                </div>
                <p v-if="externalGameDir" class="m-0 break-all text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.gameDirCurrent) }}:
                    <code>{{ externalGameDir }}</code>
                </p>
                <p v-if="!isDirectLinked" class="m-0 text-sm text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.gameDirMoveNote) }}
                </p>
                <p v-else class="m-0 text-sm text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.gameDirExternalNote) }}
                </p>
            </template>
            <p v-else class="m-0 text-sm text-[var(--color-text-tertiary)]">
                {{ formatMessage(messages.gameDirManagedNote) }}
            </p>
        </div>
        <div class="mt-6 flex flex-col gap-2.5">
            <h2 class="m-0 block text-lg font-semibold text-[var(--color-text-primary)]">
                {{ formatMessage(messages.updateChannel) }}
            </h2>
            <Chips
                v-model="selectedReleaseChannel"
                :items="releaseChannelOptions"
                :format-label="formatReleaseChannelLabel"
                :capitalize="false"
                :disabled-items="releaseChannelDisabledItems"
                :aria-label="formatMessage(messages.selectUpdateChannelAriaLabel)"
            />
            <p class="m-0">
                {{ formatReleaseChannelDescription(selectedReleaseChannel) }}
            </p>
        </div>

        <div class="mt-6 flex flex-col gap-2.5">
            <h2
                id="delete-instance-label"
                class="m-0 block text-lg font-semibold text-[var(--color-text-primary)]"
            >
                {{ formatMessage(messages.deleteInstance) }}
            </h2>
            <Button
                type="colored"
                color="red"
                aria-labelledby="delete-instance-label"
                :disabled="removing"
                class="w-fit !shadow-none"
                @click="deleteConfirmModal?.show()"
                ><SpinnerIcon v-if="removing" class="animate-spin" />
                <TrashIcon v-else />
                {{
                    removing
                        ? formatMessage(messages.deletingInstanceButton)
                        : formatMessage(messages.deleteInstanceButton)
                }}
            </Button>
            <p class="m-0">
                {{ formatMessage(messages.deleteInstanceDescription) }}
            </p>
        </div>
    </div>
</template>
