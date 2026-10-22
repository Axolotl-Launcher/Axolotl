<script setup lang="ts">
import {
    CopyIcon,
    EditIcon,
    MonitorIcon,
    SpinnerIcon,
    TrashIcon,
    UploadIcon,
} from '@modrinth/assets'
import {
    Button,
    Chips,
    commonMessages,
    defineMessages,
    injectFilePicker,
    injectNotificationManager,
    OverflowMenu,
    RadioButtons,
    StyledInput,
    useVIntl,
} from '@modrinth/ui'
import { useQueryClient } from '@tanstack/vue-query'
import { basename, dirname, join } from '@tauri-apps/api/path'
import { computed, type Ref, ref, watch } from 'vue'
import { useRouter } from 'vue-router'

import InstanceIcon from '@/components/ui/InstanceIcon.vue'
import ConfirmDeleteInstanceModal from '@/components/ui/modal/ConfirmDeleteInstanceModal.vue'
import { trackEvent } from '@/helpers/analytics'
import { install_duplicate_instance } from '@/helpers/install'
import { edit, edit_icon, get_full_path, remove } from '@/helpers/instance'
import { createInstanceShortcutOnDesktop } from '@/helpers/utils'
import { injectInstanceSettings } from '@/providers/instance-settings'

import type { GameInstance } from '../../../helpers/types'
import { INSTANCE_NAME_MAX_LENGTH, useInstanceName } from './use-instance-name'

const { addNotification, handleError } = injectNotificationManager()
const filePicker = injectFilePicker()
const { formatMessage } = useVIntl()
const router = useRouter()
const queryClient = useQueryClient()

const deleteConfirmModal = ref()

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

async function duplicateInstance() {
    await install_duplicate_instance(instance.value.id).catch(handleError)
    trackEvent('InstanceDuplicate', {
        loader: instance.value.loader,
        game_version: instance.value.game_version,
    })
}

const creatingDesktopShortcut = ref(false)
async function createDesktopShortcut() {
    if (creatingDesktopShortcut.value) return

    creatingDesktopShortcut.value = true
    try {
        await createInstanceShortcutOnDesktop(
            title.value.trim() || instance.value.name,
            instance.value.id,
        )
        addNotification({
            type: 'success',
            title: formatMessage(messages.desktopShortcutCreated),
        })
    } catch (error) {
        handleError(error)
    } finally {
        creatingDesktopShortcut.value = false
    }
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
    await edit(instanceId, { update_channel: channel })
        .then(() => queryClient.invalidateQueries({ queryKey: ['linkedModpackInfo', instanceId] }))
        .catch((error) => {
            selectedReleaseChannel.value = previousReleaseChannel
            handleError(error)
        })
    savingReleaseChannel.value = false
})

async function resetIcon() {
    icon.value = undefined
    await edit_icon(instance.value.id, null).catch(handleError)
    trackEvent('InstanceRemoveIcon')
}

async function setIcon() {
    try {
        const picked = await (filePicker.pickInstanceIcon?.() ?? filePicker.pickImage())
        if (!picked?.path) return

        const previousIcon = icon.value
        icon.value = picked.path
        try {
            await edit_icon(instance.value.id, picked.path)
            trackEvent('InstanceSetIcon')
        } catch (error) {
            icon.value = previousIcon
            handleError(error)
        }
    } catch (error) {
        handleError(error)
    }
}

const gameDirOverride = ref(instance.value.game_dir_override)
const savingGameDir = ref(false)
const isDirectLinked = computed(() => !!instance.value.linked_dot_minecraft)
const configuredExternalGameDir = computed(
    () => gameDirOverride.value ?? instance.value.linked_dot_minecraft ?? null,
)
const resolvedExternalGameDir = ref<string | null>(null)
const externalGameDir = computed(
    () => resolvedExternalGameDir.value ?? configuredExternalGameDir.value,
)

let pathResolutionRevision = 0
async function refreshExternalGameDir() {
    const revision = ++pathResolutionRevision
    if (!configuredExternalGameDir.value) {
        resolvedExternalGameDir.value = null
        return
    }
    try {
        const path = await get_full_path(instance.value.id)
        if (revision === pathResolutionRevision) resolvedExternalGameDir.value = path
    } catch {
        // Keep the configured path visible while the external chain is unavailable.
        if (revision === pathResolutionRevision) resolvedExternalGameDir.value = null
    }
}

watch(
    () => instance.value.game_dir_override,
    (path) => {
        gameDirOverride.value = path
        void refreshExternalGameDir()
    },
)

watch(
    () => [instance.value.id, instance.value.linked_dot_minecraft] as const,
    () => void refreshExternalGameDir(),
    { immediate: true },
)

// An external game dir is stored as a single path. Whether it is version
// isolated is encoded in the path: `<root>/versions/<name>` vs the `.minecraft`
// root itself. `isExternal` is false for built-in (managed) instances, which
// expose no isolation option.
const isExternal = computed(() => !!externalGameDir.value)

const gameDirInfo = ref<{ isolated: boolean; baseRoot: string | null }>({
    isolated: false,
    baseRoot: null,
})

async function refreshGameDirInfo() {
    const path = configuredExternalGameDir.value
    if (!path || isDirectLinked.value) {
        gameDirInfo.value = { isolated: false, baseRoot: null }
        return
    }
    try {
        const parent = await dirname(path)
        if ((await basename(parent)).toLowerCase() === 'versions') {
            gameDirInfo.value = { isolated: true, baseRoot: await dirname(parent) }
        } else {
            gameDirInfo.value = { isolated: false, baseRoot: path }
        }
    } catch {
        gameDirInfo.value = { isolated: false, baseRoot: path }
    }
}

watch(
    () => [configuredExternalGameDir.value, isDirectLinked.value] as const,
    () => void refreshGameDirInfo(),
    { immediate: true },
)

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
        mode === 'isolated' ? await join(baseRoot, 'versions', instance.value.name) : baseRoot
    if (nextPath === gameDirOverride.value) return

    // The launcher only records the new override path; the user is responsible
    // for actually moving the mods/saves/config folders to match.
    const previous = gameDirOverride.value
    gameDirOverride.value = nextPath
    savingGameDir.value = true
    try {
        await edit(instance.value.id, { game_dir_override: nextPath })
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

    trackEvent('InstanceRemove', {
        loader: instance.value.loader,
        game_version: instance.value.game_version,
    })

    try {
        await saveName()
        await remove(path)
        await router.push({ path: '/' })
    } finally {
        removing.value = false
    }
}

const messages = defineMessages({
    nameRequired: {
        id: 'instance.settings.general.name-required',
        defaultMessage: 'Enter a name with no more than 80 characters.',
    },
    nameSaveFailed: {
        id: 'instance.settings.general.name-save-failed',
        defaultMessage: 'The name could not be saved.',
    },
    icon: {
        id: 'instance.settings.tabs.general.icon',
        defaultMessage: 'Icon',
    },
    name: {
        id: 'instance.settings.tabs.general.name',
        defaultMessage: 'Name',
    },
    editIcon: {
        id: 'instance.settings.tabs.general.edit-icon',
        defaultMessage: 'Edit icon',
    },
    selectIcon: {
        id: 'instance.settings.tabs.general.edit-icon.select',
        defaultMessage: 'Select icon',
    },
    replaceIcon: {
        id: 'instance.settings.tabs.general.edit-icon.replace',
        defaultMessage: 'Replace icon',
    },
    removeIcon: {
        id: 'instance.settings.tabs.general.edit-icon.remove',
        defaultMessage: 'Remove icon',
    },
    duplicateInstance: {
        id: 'instance.settings.tabs.general.duplicate-instance',
        defaultMessage: 'Duplicate instance',
    },
    duplicateInstanceDescription: {
        id: 'instance.settings.tabs.general.duplicate-instance.description',
        defaultMessage: 'Creates a copy of this instance, including worlds, configs, mods, etc.',
    },
    duplicateButtonTooltipInstalling: {
        id: 'instance.settings.tabs.general.duplicate-button.tooltip.installing',
        defaultMessage: 'Cannot duplicate while installing.',
    },
    duplicateButton: {
        id: 'instance.settings.tabs.general.duplicate-button',
        defaultMessage: 'Duplicate',
    },
    desktopShortcut: {
        id: 'instance.settings.tabs.general.desktop-shortcut',
        defaultMessage: 'Desktop shortcut',
    },
    desktopShortcutDescription: {
        id: 'instance.settings.tabs.general.desktop-shortcut.description',
        defaultMessage: 'Create a desktop shortcut that launches this instance directly.',
    },
    createDesktopShortcut: {
        id: 'instance.settings.tabs.general.desktop-shortcut.create',
        defaultMessage: 'Create desktop shortcut',
    },
    creatingDesktopShortcut: {
        id: 'instance.settings.tabs.general.desktop-shortcut.creating',
        defaultMessage: 'Creating shortcut...',
    },
    desktopShortcutCreated: {
        id: 'instance.settings.tabs.general.desktop-shortcut.created',
        defaultMessage: 'Desktop shortcut created',
    },
    gameDir: {
        id: 'instance.settings.tabs.general.game-dir',
        defaultMessage: 'Game directory',
    },
    gameDirDescription: {
        id: 'instance.settings.tabs.general.game-dir.description',
        defaultMessage:
            'Uses a separate folder as the working directory for this instance. The game reads mods, saves, configs, and resource packs from that folder instead of the managed instance folder.',
    },
    gameDirCurrent: {
        id: 'instance.settings.tabs.general.game-dir.current',
        defaultMessage: 'Current directory',
    },
    gameDirIsolated: {
        id: 'instance.settings.tabs.general.game-dir.isolated',
        defaultMessage: 'Version isolated (stored in versions/)',
    },
    gameDirNotIsolated: {
        id: 'instance.settings.tabs.general.game-dir.not-isolated',
        defaultMessage: 'Version shared (.minecraft/)',
    },
    gameDirMoveNote: {
        id: 'instance.settings.tabs.general.game-dir.move-note',
        defaultMessage:
            'Switching isolation only updates the launcher path. Move the mods, saves, and config folders yourself to match.',
    },
    gameDirManagedNote: {
        id: 'instance.settings.tabs.general.game-dir.managed-note',
        defaultMessage: 'This instance uses the Axolotl-managed folder.',
    },
    gameDirExternalNote: {
        id: 'instance.settings.tabs.general.game-dir.external-note',
        defaultMessage: 'This instance uses an external .minecraft folder managed in place.',
    },
    updateChannel: {
        id: 'instance.settings.tabs.general.update-channel',
        defaultMessage: 'Update channel',
    },
    updateChannelReleaseDescription: {
        id: 'instance.settings.tabs.general.update-channel.release.description',
        defaultMessage: 'Only release versions will be shown as available updates.',
    },
    updateChannelBetaDescription: {
        id: 'instance.settings.tabs.general.update-channel.beta.description',
        defaultMessage: 'Release and beta versions will be shown as available updates.',
    },
    updateChannelAlphaDescription: {
        id: 'instance.settings.tabs.general.update-channel.alpha.description',
        defaultMessage: 'Release, beta, and alpha versions will be shown as available updates.',
    },
    updateChannelRelease: {
        id: 'instance.settings.tabs.general.update-channel.release',
        defaultMessage: 'Release',
    },
    updateChannelBeta: {
        id: 'instance.settings.tabs.general.update-channel.beta',
        defaultMessage: 'Beta',
    },
    updateChannelAlpha: {
        id: 'instance.settings.tabs.general.update-channel.alpha',
        defaultMessage: 'Alpha',
    },
    selectUpdateChannelAriaLabel: {
        id: 'instance.settings.tabs.general.update-channel.select',
        defaultMessage: 'Select update channel',
    },
    deleteInstance: {
        id: 'instance.settings.tabs.general.delete',
        defaultMessage: 'Delete instance',
    },
    deleteInstanceDescription: {
        id: 'instance.settings.tabs.general.delete.description',
        defaultMessage:
            'Permanently deletes an instance from your device, including your worlds, configs, and all installed content. Be careful, as once you delete a instance there is no way to recover it.',
    },
    deleteInstanceButton: {
        id: 'instance.settings.tabs.general.delete.button',
        defaultMessage: 'Delete instance',
    },
    deletingInstanceButton: {
        id: 'instance.settings.tabs.general.deleting.button',
        defaultMessage: 'Deleting...',
    },
})
</script>

<template>
    <ConfirmDeleteInstanceModal
        ref="deleteConfirmModal"
        :instance-id="instance.id"
        :symlink-target="instance.symlink_target"
        :instances="[instance]"
        :delete-action="removeInstance"
    />
    <div class="block">
        <div class="float-end ml-10 relative group w-fit">
            <div class="flex flex-col gap-1">
                <span class="text-lg font-semibold text-[var(--color-text-primary)]">
                    {{ formatMessage(messages.icon) }}
                </span>
                <div class="group relative w-fit">
                    <OverflowMenu
                        v-tooltip="formatMessage(messages.editIcon)"
                        class="bg-transparent border-none appearance-none p-0 m-0 cursor-pointer group-active:scale-95 transition-transform"
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
                            class="absolute top-0 h-full w-full flex items-center justify-center opacity-0 transition-all group-hover:opacity-100"
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
            class="m-0 text-lg font-semibold text-[var(--color-text-primary)] block"
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
            <div class="flex flex-col gap-2.5 mt-6">
                <h2
                    id="duplicate-instance-label"
                    class="m-0 text-lg font-semibold text-[var(--color-text-primary)] block"
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
        <div class="flex flex-col gap-2.5 mt-6">
            <h2
                id="desktop-shortcut-label"
                class="m-0 text-lg font-semibold text-[var(--color-text-primary)] block"
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
        <div class="flex flex-col gap-2.5 mt-6">
            <h2 class="m-0 text-lg font-semibold text-[var(--color-text-primary)] block">
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
                <p v-if="externalGameDir" class="m-0 text-[var(--color-text-tertiary)] break-all">
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
        <div class="flex flex-col gap-2.5 mt-6">
            <h2 class="m-0 text-lg font-semibold text-[var(--color-text-primary)] block">
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

        <div class="flex flex-col gap-2.5 mt-6">
            <h2
                id="delete-instance-label"
                class="m-0 text-lg font-semibold text-[var(--color-text-primary)] block"
            >
                {{ formatMessage(messages.deleteInstance) }}
            </h2>
            <Button
                type="colored"
                color="red"
                aria-labelledby="delete-instance-label"
                :disabled="removing"
                class="w-fit !shadow-none"
                @click="deleteConfirmModal.show()"
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
