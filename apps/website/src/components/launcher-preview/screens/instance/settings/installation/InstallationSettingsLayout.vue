<script setup lang="ts">
import ArrowLeftRightIcon from '@modrinth/assets/icons/arrow-left-right.svg?component'
import CircleAlertIcon from '@modrinth/assets/icons/circle-alert.svg?component'
import DownloadIcon from '@modrinth/assets/icons/download.svg?component'
import EyeIcon from '@modrinth/assets/icons/eye.svg?component'
import EyeOffIcon from '@modrinth/assets/icons/eye-off.svg?component'
import HammerIcon from '@modrinth/assets/icons/hammer.svg?component'
import PencilIcon from '@modrinth/assets/icons/pencil.svg?component'
import SaveIcon from '@modrinth/assets/icons/save.svg?component'
import SpinnerIcon from '@modrinth/assets/icons/spinner.svg?component'
import UnlinkIcon from '@modrinth/assets/icons/unlink.svg?component'
import XIcon from '@modrinth/assets/icons/x.svg?component'
import AutoLink from '@modrinth/ui/src/components/base/AutoLink.vue'
import Avatar from '@modrinth/ui/src/components/base/Avatar.vue'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import Chips from '@modrinth/ui/src/components/base/Chips.vue'
import Combobox from '@modrinth/ui/src/components/base/Combobox.vue'
import PaperChannelBadge from '@modrinth/ui/src/components/base/PaperChannelBadge.vue'
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { useInstallationForm } from '@modrinth/ui/src/layouts/shared/installation-settings/composables/use-installation-form.ts'
import { injectInstallationSettings } from '@modrinth/ui/src/layouts/shared/installation-settings/providers/installation-settings.ts'
import type { LoaderVersionEntry } from '@modrinth/ui/src/layouts/shared/installation-settings/types.ts'
import { commonMessages } from '@modrinth/ui/src/utils/common-messages.ts'
import { formatLoaderLabel } from '@modrinth/ui/src/utils/loaders.ts'
import { versionChangesGameVersion } from '@modrinth/ui/src/utils/version-compatibility.ts'
import { computed, ref } from 'vue'
import { onBeforeRouteLeave } from 'vue-router'

import ConfirmModpackUpdateModal from '../../../../shared/content-tab/components/modals/ConfirmModpackUpdateModal.vue'
import ConfirmReinstallModal from '../../../../shared/content-tab/components/modals/ConfirmReinstallModal.vue'
import ConfirmRepairModal from '../../../../shared/content-tab/components/modals/ConfirmRepairModal.vue'
import ConfirmUnlinkModal from '../../../../shared/content-tab/components/modals/ConfirmUnlinkModal.vue'
import ContentUpdaterModal, {
    type UpdaterVersion,
} from '../../../../shared/content-tab/components/modals/ContentUpdaterModal.vue'
import ConfirmLeaveModal from './ConfirmLeaveModal.vue'

/**
 * Port of `packages/ui/src/layouts/shared/installation-settings/layout.vue` with
 * its modals rendered inside the launcher window. The app's Installation tab
 * provides no content preview or bulk-disable callbacks, so the layout's
 * content-diff and incompatible-content modals (which only open through them)
 * are not part of this port. The debug logging is left out.
 */
const { formatMessage } = useVIntl()
const ctx = injectInstallationSettings()
const skipNonEssentialWarnings = computed(() => ctx.skipNonEssentialWarnings?.value ?? false)
const availablePlatforms = computed(() =>
    Array.isArray(ctx.availablePlatforms) ? ctx.availablePlatforms : ctx.availablePlatforms.value,
)

const confirmLeaveModal = ref<InstanceType<typeof ConfirmLeaveModal>>()
const repairModal = ref<InstanceType<typeof ConfirmRepairModal>>()
const reinstallModal = ref<InstanceType<typeof ConfirmReinstallModal>>()
const unlinkModal = ref<InstanceType<typeof ConfirmUnlinkModal>>()
const contentUpdaterModal = ref<InstanceType<typeof ContentUpdaterModal> | null>()

const modpackUpdateModal = ref<InstanceType<typeof ConfirmModpackUpdateModal>>()
const pendingUpdateVersion = ref<UpdaterVersion | null>(null)
const isUpdateDowngrade = ref(false)

const form = useInstallationForm(ctx, contentUpdaterModal as never)

function paperLoaderChannelTag(index: number): LoaderVersionEntry['channelTag'] | null {
    if (form.selectedPlatform.value !== 'paper') return null
    const entries = ctx.resolveLoaderVersions(
        form.selectedPlatform.value,
        form.selectedGameVersion.value,
    )
    const tag = entries[index]?.channelTag
    return tag === 'ALPHA' || tag === 'BETA' ? tag : null
}

onBeforeRouteLeave(async () => {
    if (form.isSaving.value) {
        return (await confirmLeaveModal.value?.prompt()) ?? false
    }
    return true
})

const disabledPlatforms = computed(() => {
    if (!ctx.lockPlatform || ctx.currentPlatform.value === 'vanilla') return []
    return availablePlatforms.value.filter((p) => p !== ctx.currentPlatform.value)
})
const platformDisabledItems = computed(() =>
    ctx.isBusy.value ? availablePlatforms.value : disabledPlatforms.value,
)
const platformDisabledTooltip = computed(() =>
    ctx.isBusy.value
        ? (ctx.busyMessage?.value ?? undefined)
        : formatMessage(messages.platformLockTooltip),
)

const showModpackVersionActions = computed(() => {
    const val = ctx.showModpackVersionActions
    if (val == null) return true
    return typeof val === 'boolean' ? val : val.value
})

const isLocalFile = computed(() => {
    const val = ctx.isLocalFile
    if (val == null) return false
    return typeof val === 'boolean' ? val : val.value
})

const loaderVersionState = computed(() => ctx.loaderVersionState?.value ?? 'supported')
const loaderVersionPlaceholder = computed(() => {
    if (loaderVersionState.value === 'loading') {
        return formatMessage(commonMessages.loadingLabel)
    }
    if (loaderVersionState.value === 'unsupported') {
        return formatMessage(messages.unsupportedLoaderVersion)
    }
    if (loaderVersionState.value === 'error') {
        return formatMessage(messages.loaderVersionsError)
    }
    return formatMessage(commonMessages.selectVersionPlaceholder)
})
const loaderVersionDisabled = computed(
    () =>
        ctx.isBusy.value ||
        loaderVersionState.value === 'unknown' ||
        loaderVersionState.value === 'loading' ||
        loaderVersionState.value === 'unsupported' ||
        loaderVersionState.value === 'error',
)

const isLinkedModpack = computed(() => showModpackVersionActions.value || isLocalFile.value)

function handleModpackUpdateRequest(version: UpdaterVersion, event?: MouseEvent) {
    if (ctx.isBusy.value) return
    pendingUpdateVersion.value = version

    const currentVersionId = ctx.updaterModalProps.value.currentVersionId
    const currentVersion = form.updatingProjectVersions.value.find((v) => v.id === currentVersionId)
    isUpdateDowngrade.value = currentVersion
        ? new Date(version.date_published) < new Date(currentVersion.date_published)
        : false
    const shouldShowWarning =
        isUpdateDowngrade.value ||
        versionChangesGameVersion(version, ctx.updaterModalProps.value.currentGameVersion)

    if (event?.shiftKey || skipNonEssentialWarnings.value || !shouldShowWarning) {
        handleModpackUpdateConfirm()
        return
    }

    modpackUpdateModal.value?.show()
}

function handleSwapModpack() {
    if (ctx.isBusy.value) return
    form.cancelEditing()
    ctx.swapModpack?.()
}

function handleModpackPrimaryAction() {
    if (showModpackVersionActions.value) {
        form.handleChangeModpackVersion()
    } else {
        handleSwapModpack()
    }
}

async function handleModpackUpdateConfirm() {
    if (ctx.isBusy.value) return
    const version = pendingUpdateVersion.value
    if (version) {
        pendingUpdateVersion.value = null
        contentUpdaterModal.value?.hide()
        form.cancelEditing()
        await form.handleUpdaterConfirm(version as never)
        ctx.closeSettings?.()
    }
}

function handleModpackUpdateCancel() {
    pendingUpdateVersion.value = null
}

function handleRepair() {
    if (ctx.isBusy.value) return
    form.cancelEditing()
    ctx.repair()
}

function handleReinstall() {
    if (ctx.isBusy.value) return
    form.cancelEditing()
    ctx.reinstallModpack()
}

async function handleUnlink() {
    if (ctx.isBusy.value) return
    form.cancelEditing()
    await ctx.unlinkModpack()
}

function handleStartEditing() {
    form.isEditing.value = true
}

function handleCancelEditing() {
    form.cancelEditing()
}

function handleSave() {
    void form.save()
}

function handleShowRepairModal() {
    if (skipNonEssentialWarnings.value) {
        handleRepair()
        return
    }
    repairModal.value?.show()
}

function handleShowUnlinkModal(event: MouseEvent) {
    if (event.shiftKey || skipNonEssentialWarnings.value) {
        handleUnlink()
        return
    }
    unlinkModal.value?.show()
}

function handleShowReinstallModal(event: MouseEvent) {
    if (event.shiftKey) {
        handleReinstall()
        return
    }
    reinstallModal.value?.show()
}

defineExpose({
    cancelEditing: () => form.cancelEditing(),
})

const messages = defineMessages({
    linkedInstanceTitle: {
        id: 'installation-settings.linked-instance.title',
        defaultMessage: 'Linked {projectType}',
    },
    reinstallModpackTitle: {
        id: 'installation-settings.reinstall-modpack.title',
        defaultMessage: 'Re-install modpack',
    },
    reinstallModpackDescription: {
        id: 'installation-settings.reinstall-modpack.description',
        defaultMessage:
            'Re-installing the modpack resets the {type} content to its original state, removing any mods or content you have added.',
    },
    editInstallationTitle: {
        id: 'installation-settings.edit-installation.title',
        defaultMessage: 'Edit installation',
    },
    unlinkDescription: {
        id: 'installation-settings.unlink.description',
        defaultMessage:
            "Unlinking permanently disconnects this {type} from the {projectType} project, allowing you to change the loader and Minecraft version, but you won't receive future updates.",
    },
    repairInstanceTitle: {
        id: 'installation-settings.repair.instance-title',
        defaultMessage: 'Repair instance',
    },
    repairInstanceDescription: {
        id: 'installation-settings.repair.instance-description',
        defaultMessage:
            'Reinstalls Minecraft dependencies and checks for corruption. This may resolve issues if your game is not launching due to launcher-related errors.',
    },
    repairServerTitle: {
        id: 'installation-settings.repair.server-title',
        defaultMessage: 'Repair server',
    },
    repairServerDescription: {
        id: 'installation-settings.repair.server-description',
        defaultMessage:
            'Reinstalls the loader and Minecraft dependencies without deleting your content. This may resolve issues if your server is not starting correctly.',
    },
    editWarningInstance: {
        id: 'installation-settings.edit.warning-instance',
        defaultMessage:
            "We don't recommend editing your installation settings after installing content. If you want to edit them, be cautious as it may cause issues.",
    },
    serverProjectLabel: {
        id: 'installation-settings.linked.server-project',
        defaultMessage: 'server project',
    },
    modpackLabel: {
        id: 'installation-settings.linked.modpack',
        defaultMessage: 'modpack',
    },
    serverLabel: {
        id: 'installation-settings.type.server',
        defaultMessage: 'server',
    },
    instanceLabel: {
        id: 'installation-settings.type.instance',
        defaultMessage: 'instance',
    },
    serverPossessiveLabel: {
        id: 'installation-settings.type.server-possessive',
        defaultMessage: "server's",
    },
    instancePossessiveLabel: {
        id: 'installation-settings.type.instance-possessive',
        defaultMessage: "instance's",
    },
    editWarningServer: {
        id: 'installation-settings.edit.warning-server',
        defaultMessage:
            "We don't recommend editing your installation settings after installing content. If you want to edit them reset your server.",
    },
    loaderVersionLabel: {
        id: 'installation-settings.loader-version',
        defaultMessage: '{loader} version',
    },
    unsupportedLoaderVersion: {
        id: 'installation-settings.loader-version.unsupported',
        defaultMessage: 'This loader does not support the selected game version',
    },
    loaderVersionsError: {
        id: 'installation-settings.loader-version.error',
        defaultMessage: 'Failed to load loader versions',
    },
    searchGameVersionPlaceholder: {
        id: 'installation-settings.search-game-version',
        defaultMessage: 'Search game version...',
    },
    savingLabel: {
        id: 'installation-settings.saving',
        defaultMessage: 'Saving...',
    },
    verifyingLabel: {
        id: 'installation-settings.verifying',
        defaultMessage: 'Verifying...',
    },
    selectPlatformAriaLabel: {
        id: 'installation-settings.aria.select-platform',
        defaultMessage: 'Select platform',
    },
    selectGameVersionAriaLabel: {
        id: 'installation-settings.aria.select-game-version',
        defaultMessage: 'Select game version',
    },
    selectLoaderVersionAriaLabel: {
        id: 'installation-settings.aria.select-loader-version',
        defaultMessage: 'Select {loader} version',
    },
    reinstallingModpackButton: {
        id: 'installation-settings.reinstalling-modpack',
        defaultMessage: 'Reinstalling modpack',
    },
    unlinkButton: {
        id: 'installation-settings.unlink',
        defaultMessage: 'Unlink',
    },
    platformLockTooltip: {
        id: 'installation-settings.platform-lock-tooltip',
        defaultMessage: 'You will need to reset your server to switch loader.',
    },
})
</script>

<template>
    <div class="flex flex-col gap-6">
        <!-- Loading state -->
        <div v-if="ctx.loading.value" class="flex items-center justify-center py-12">
            <SpinnerIcon class="size-8 animate-spin text-[var(--color-text-tertiary)]" />
        </div>

        <template v-else>
            <!-- Installation Info (linked state) -->
            <div v-if="ctx.isLinked.value" class="flex flex-col gap-2.5">
                <span class="text-lg font-semibold text-[var(--color-text-primary)]">
                    {{ formatMessage(commonMessages.installationInfoTitle) }}
                </span>
                <div class="flex flex-col gap-2.5 rounded-[var(--radius-xl)] bg-surface-2 p-4">
                    <div
                        v-for="row in ctx.installationInfo.value"
                        :key="row.label"
                        class="flex items-center justify-between"
                    >
                        <span class="text-[var(--color-text-default)]">{{ row.label }}</span>
                        <span
                            v-if="row.value"
                            class="font-semibold text-[var(--color-text-primary)]"
                            >{{ row.value }}</span
                        >
                        <span
                            v-else
                            class="inline-block h-3 w-16 animate-pulse rounded bg-button-border"
                        ></span>
                    </div>
                </div>
            </div>

            <!-- LINKED -->
            <template v-if="ctx.isLinked.value">
                <!-- Installed Modpack -->
                <div class="flex flex-col gap-2.5">
                    <span class="text-lg font-semibold text-[var(--color-text-primary)]">
                        {{ formatMessage(commonMessages.installedModpackTitle) }}
                    </span>
                    <div
                        v-if="ctx.modpack.value"
                        class="flex items-center gap-2.5 rounded-[var(--radius-xl)] bg-surface-2 p-3"
                    >
                        <AutoLink :to="ctx.modpack.value.link" class="shrink-0">
                            <Avatar
                                :src="ctx.modpack.value.iconUrl"
                                :alt="ctx.modpack.value.title"
                                size="3.5rem"
                                no-shadow
                                raised
                            />
                        </AutoLink>
                        <div class="flex min-w-0 flex-col gap-1">
                            <div class="flex min-w-0 flex-col">
                                <AutoLink
                                    :to="ctx.modpack.value.link"
                                    class="truncate font-semibold text-[var(--color-text-primary)]"
                                    :class="ctx.modpack.value.link ? 'hover:underline' : ''"
                                >
                                    {{ ctx.modpack.value.title }}
                                </AutoLink>
                                <span
                                    v-if="ctx.modpack.value.filename"
                                    class="truncate text-sm text-[var(--color-text-tertiary)]"
                                >
                                    {{ ctx.modpack.value.filename }}
                                </span>
                            </div>
                            <div
                                v-if="ctx.modpack.value.owner || ctx.modpack.value.versionNumber"
                                class="flex items-center gap-2 text-sm text-[var(--color-text-tertiary)]"
                            >
                                <AutoLink
                                    v-if="ctx.modpack.value.owner"
                                    :to="
                                        ctx.modpack.value.owner.type === 'organization'
                                            ? `/organization/${ctx.modpack.value.owner.id}`
                                            : `/user/${ctx.modpack.value.owner.id}`
                                    "
                                    class="flex items-center gap-1.5 hover:underline"
                                >
                                    <Avatar
                                        :src="ctx.modpack.value.owner.iconUrl"
                                        :alt="ctx.modpack.value.owner.name"
                                        size="1.25rem"
                                        :circle="ctx.modpack.value.owner.type === 'user'"
                                        no-shadow
                                    />
                                    <span class="font-medium">{{
                                        ctx.modpack.value.owner.name
                                    }}</span>
                                </AutoLink>
                                <template
                                    v-if="
                                        ctx.modpack.value.owner && ctx.modpack.value.versionNumber
                                    "
                                >
                                    &middot;
                                </template>
                                <span v-if="ctx.modpack.value.versionNumber" class="font-medium">
                                    {{ ctx.modpack.value.versionNumber }}
                                </span>
                            </div>
                        </div>
                    </div>
                    <div class="flex flex-wrap gap-2">
                        <Button
                            v-if="showModpackVersionActions || isLocalFile"
                            v-tooltip="ctx.isBusy.value ? ctx.busyMessage?.value : undefined"
                            class="!shadow-none"
                            :disabled="ctx.isBusy.value"
                            @click="handleModpackPrimaryAction"
                            ><ArrowLeftRightIcon class="size-5" />
                            {{ formatMessage(commonMessages.changeVersionButton) }}
                        </Button>
                    </div>
                </div>

                <!-- Unlink -->
                <div class="flex flex-col gap-2.5">
                    <span class="text-lg font-semibold text-[var(--color-text-primary)]">
                        {{
                            formatMessage(messages.linkedInstanceTitle, {
                                projectType: formatMessage(
                                    isLinkedModpack
                                        ? messages.modpackLabel
                                        : messages.serverProjectLabel,
                                ),
                            })
                        }}
                    </span>
                    <div>
                        <Button
                            v-tooltip="ctx.isBusy.value ? ctx.busyMessage?.value : undefined"
                            type="colored"
                            color="orange"
                            class="!shadow-none"
                            :disabled="ctx.isBusy.value"
                            @click="handleShowUnlinkModal"
                            ><UnlinkIcon class="size-5" />
                            {{
                                formatMessage(
                                    isLinkedModpack
                                        ? commonMessages.unlinkModpackButton
                                        : messages.unlinkButton,
                                )
                            }}
                        </Button>
                    </div>
                    <span class="text-[var(--color-text-default)]">
                        {{
                            formatMessage(messages.unlinkDescription, {
                                type: formatMessage(
                                    ctx.isServer ? messages.serverLabel : messages.instanceLabel,
                                ),
                                projectType: formatMessage(
                                    isLinkedModpack ? messages.modpackLabel : messages.serverLabel,
                                ),
                            })
                        }}
                    </span>
                </div>

                <!-- Reinstall -->
                <div v-if="showModpackVersionActions || isLocalFile" class="flex flex-col gap-2.5">
                    <span class="text-lg font-semibold text-[var(--color-text-primary)]">
                        {{ formatMessage(messages.reinstallModpackTitle) }}
                    </span>
                    <div>
                        <Button
                            v-tooltip="ctx.isBusy.value ? ctx.busyMessage?.value : undefined"
                            type="colored"
                            color="red"
                            class="!shadow-none"
                            :disabled="ctx.isBusy.value"
                            @click="handleShowReinstallModal"
                            ><SpinnerIcon v-if="ctx.reinstalling?.value" class="animate-spin" />
                            <DownloadIcon v-else class="size-5" />
                            {{
                                ctx.reinstalling?.value
                                    ? formatMessage(messages.reinstallingModpackButton)
                                    : formatMessage(commonMessages.reinstallModpackButton)
                            }}
                        </Button>
                    </div>
                    <span class="text-[var(--color-text-default)]">
                        {{
                            formatMessage(messages.reinstallModpackDescription, {
                                type: formatMessage(
                                    ctx.isServer
                                        ? messages.serverPossessiveLabel
                                        : messages.instancePossessiveLabel,
                                ),
                            })
                        }}
                    </span>
                </div>

                <!-- Repair (hidden for local file modpacks — reinstall covers this) -->
                <div v-if="!isLocalFile" class="flex flex-col gap-2.5">
                    <span class="text-lg font-semibold text-[var(--color-text-primary)]">
                        {{
                            formatMessage(
                                ctx.isServer
                                    ? messages.repairServerTitle
                                    : messages.repairInstanceTitle,
                            )
                        }}
                    </span>
                    <div>
                        <Button
                            v-tooltip="ctx.isBusy.value ? ctx.busyMessage?.value : undefined"
                            class="!shadow-none"
                            :disabled="ctx.isBusy.value"
                            @click="handleShowRepairModal"
                            ><SpinnerIcon v-if="ctx.repairing?.value" class="animate-spin" />
                            <HammerIcon v-else class="size-5" />
                            {{
                                ctx.repairing?.value
                                    ? formatMessage(commonMessages.repairingButton)
                                    : formatMessage(commonMessages.repairButton)
                            }}
                        </Button>
                    </div>
                    <span class="text-[var(--color-text-default)]">
                        {{
                            formatMessage(
                                ctx.isServer
                                    ? messages.repairServerDescription
                                    : messages.repairInstanceDescription,
                            )
                        }}
                    </span>
                </div>
            </template>

            <!-- NOT LINKED -->
            <template v-else>
                <!-- Edit form -->
                <div v-if="form.isEditing.value" class="flex flex-col gap-2.5">
                    <span class="text-lg font-semibold text-[var(--color-text-primary)]">
                        {{ formatMessage(messages.editInstallationTitle) }}
                    </span>
                    <div
                        class="flex flex-col gap-3 rounded-[var(--radius-xl)] border border-solid border-surface-5 p-4"
                    >
                        <div class="flex flex-col gap-2.5">
                            <span class="font-semibold text-[var(--color-text-primary)]">
                                {{ formatMessage(commonMessages.platformLabel) }}
                            </span>
                            <Chips
                                v-model="form.selectedPlatform.value"
                                :items="availablePlatforms"
                                :format-label="formatLoaderLabel"
                                :capitalize="false"
                                :disabled-items="platformDisabledItems"
                                :disabled-tooltip="platformDisabledTooltip"
                                :aria-label="formatMessage(messages.selectPlatformAriaLabel)"
                            />
                        </div>

                        <div class="flex flex-col gap-2.5">
                            <span class="font-semibold text-[var(--color-text-primary)]">
                                {{ formatMessage(commonMessages.gameVersionLabel) }}
                            </span>
                            <Combobox
                                v-model="form.selectedGameVersion.value"
                                v-tooltip="ctx.isBusy.value ? ctx.busyMessage?.value : undefined"
                                :options="form.gameVersionOptions.value"
                                searchable
                                sync-with-selection
                                :placeholder="
                                    formatMessage(commonMessages.selectVersionPlaceholder)
                                "
                                :search-placeholder="
                                    formatMessage(messages.searchGameVersionPlaceholder)
                                "
                                :display-value="
                                    form.selectedGameVersion.value ||
                                    formatMessage(commonMessages.selectVersionPlaceholder)
                                "
                                :aria-label="formatMessage(messages.selectGameVersionAriaLabel)"
                                :disabled="ctx.isBusy.value"
                                @option-hover="ctx.onGameVersionHover?.($event)"
                            >
                                <template v-if="form.hasSnapshots.value" #dropdown-footer>
                                    <button
                                        v-tooltip="
                                            ctx.isBusy.value ? ctx.busyMessage?.value : undefined
                                        "
                                        class="flex w-full cursor-pointer items-center justify-center gap-1.5 border-0 border-t border-solid border-surface-5 bg-transparent py-3 text-center text-sm font-semibold text-[var(--color-text-tertiary)] transition-colors hover:text-[var(--color-text-primary)]"
                                        :disabled="ctx.isBusy.value"
                                        @mousedown.prevent
                                        @click="
                                            form.showSnapshots.value = !form.showSnapshots.value
                                        "
                                    >
                                        <EyeOffIcon
                                            v-if="form.showSnapshots.value"
                                            class="size-4"
                                        />
                                        <EyeIcon v-else class="size-4" />
                                        {{
                                            form.showSnapshots.value
                                                ? formatMessage(commonMessages.hideSnapshotsButton)
                                                : formatMessage(
                                                      commonMessages.showAllVersionsButton,
                                                  )
                                        }}
                                    </button>
                                </template>
                            </Combobox>
                        </div>

                        <div
                            v-if="
                                form.selectedPlatform.value !== 'vanilla' && !ctx.hideLoaderVersion
                            "
                            class="flex flex-col gap-2.5"
                        >
                            <span class="font-semibold text-[var(--color-text-primary)]">
                                {{
                                    formatMessage(messages.loaderVersionLabel, {
                                        loader: form.formattedLoaderName.value,
                                    })
                                }}
                            </span>
                            <Combobox
                                v-model="form.selectedLoaderVersion.value"
                                v-tooltip="ctx.isBusy.value ? ctx.busyMessage?.value : undefined"
                                searchable
                                sync-with-selection
                                :placeholder="
                                    loaderVersionState === 'supported'
                                        ? form.loaderVersionDisplayValue.value ||
                                          loaderVersionPlaceholder
                                        : loaderVersionPlaceholder
                                "
                                :search-placeholder="
                                    loaderVersionState === 'supported'
                                        ? formatMessage(commonMessages.searchVersionPlaceholder)
                                        : loaderVersionPlaceholder
                                "
                                :options="form.loaderVersionOptions.value"
                                :display-value="
                                    loaderVersionState === 'supported'
                                        ? form.loaderVersionDisplayValue.value ||
                                          loaderVersionPlaceholder
                                        : loaderVersionPlaceholder
                                "
                                :aria-label="
                                    formatMessage(messages.selectLoaderVersionAriaLabel, {
                                        loader: form.formattedLoaderName.value,
                                    })
                                "
                                :disabled="loaderVersionDisabled"
                            >
                                <template
                                    v-if="form.selectedPlatform.value === 'paper'"
                                    #option="{ item, isSelected }"
                                >
                                    <div class="flex w-full items-center justify-between gap-2">
                                        <div class="flex flex-wrap items-center gap-2">
                                            <span
                                                class="font-semibold leading-tight"
                                                :class="
                                                    isSelected
                                                        ? 'text-[var(--color-text-primary)]'
                                                        : 'text-[var(--color-text-default)]'
                                                "
                                            >
                                                {{ item.label }}
                                            </span>
                                            <PaperChannelBadge
                                                :channel="paperLoaderChannelTag(item.value)"
                                            />
                                        </div>
                                    </div>
                                </template>
                                <template
                                    v-if="form.selectedPlatform.value === 'paper'"
                                    #search-selection-affix="{ option }"
                                >
                                    <PaperChannelBadge
                                        affix
                                        :channel="
                                            option ? paperLoaderChannelTag(option.value) : null
                                        "
                                    />
                                </template>
                            </Combobox>
                        </div>

                        <div class="flex flex-wrap gap-2">
                            <Button
                                v-tooltip="ctx.isBusy.value ? ctx.busyMessage?.value : undefined"
                                type="colored"
                                color="brand"
                                class="!shadow-none"
                                :disabled="
                                    !form.isValid.value ||
                                    !form.hasChanges.value ||
                                    form.isSaving.value ||
                                    ctx.isBusy.value
                                "
                                @click="handleSave"
                                ><SpinnerIcon v-if="form.isSaving.value" class="animate-spin" />
                                <SaveIcon v-else />
                                {{
                                    form.isVerifying.value
                                        ? formatMessage(messages.verifyingLabel)
                                        : form.isSaving.value
                                          ? formatMessage(messages.savingLabel)
                                          : formatMessage(commonMessages.saveButton)
                                }}
                            </Button>
                            <Button type="outlined" @click="handleCancelEditing"
                                ><XIcon />
                                {{ formatMessage(commonMessages.cancelButton) }}
                            </Button>
                        </div>
                    </div>
                </div>

                <!-- Non-editing: installation info + warning + edit button -->
                <div v-if="!form.isEditing.value" class="flex flex-col gap-2.5">
                    <span class="text-lg font-semibold text-[var(--color-text-primary)]">
                        {{ formatMessage(commonMessages.installationInfoTitle) }}
                    </span>
                    <div class="flex flex-col gap-2.5 rounded-[var(--radius-xl)] bg-surface-2 p-4">
                        <div
                            v-for="row in ctx.installationInfo.value"
                            :key="row.label"
                            class="flex items-center justify-between"
                        >
                            <span class="text-[var(--color-text-default)]">{{ row.label }}</span>
                            <span class="font-semibold text-[var(--color-text-primary)]">{{
                                row.value
                            }}</span>
                        </div>
                    </div>
                    <div class="flex flex-wrap gap-2">
                        <Button
                            v-tooltip="ctx.isBusy.value ? ctx.busyMessage?.value : undefined"
                            type="colored"
                            color="orange"
                            class="!shadow-none"
                            :disabled="ctx.isBusy.value"
                            @click="handleStartEditing"
                            ><PencilIcon class="size-5" />
                            {{ formatMessage(commonMessages.editButton) }}
                        </Button>
                        <slot name="unlinked-extra-buttons" />
                    </div>
                    <div class="flex items-start gap-2">
                        <CircleAlertIcon class="mt-0.5 size-5 shrink-0 text-orange" />
                        <span class="text-[var(--color-text-default)]">
                            {{
                                formatMessage(
                                    ctx.isServer
                                        ? messages.editWarningServer
                                        : messages.editWarningInstance,
                                )
                            }}
                        </span>
                    </div>
                </div>

                <!-- Repair section -->
                <div v-if="ctx.currentPlatform.value !== 'vanilla'" class="flex flex-col gap-2.5">
                    <span class="text-lg font-semibold text-[var(--color-text-primary)]">
                        {{
                            formatMessage(
                                ctx.isServer
                                    ? messages.repairServerTitle
                                    : messages.repairInstanceTitle,
                            )
                        }}
                    </span>
                    <div>
                        <Button
                            v-tooltip="ctx.isBusy.value ? ctx.busyMessage?.value : undefined"
                            class="!shadow-none"
                            :disabled="ctx.isBusy.value"
                            @click="handleShowRepairModal"
                            ><SpinnerIcon v-if="ctx.repairing?.value" class="animate-spin" />
                            <HammerIcon v-else class="size-5" />
                            {{
                                ctx.repairing?.value
                                    ? formatMessage(commonMessages.repairingButton)
                                    : formatMessage(commonMessages.repairButton)
                            }}
                        </Button>
                    </div>
                    <span class="text-[var(--color-text-default)]">
                        {{
                            formatMessage(
                                ctx.isServer
                                    ? messages.repairServerDescription
                                    : messages.repairInstanceDescription,
                            )
                        }}
                    </span>
                </div>
            </template>

            <slot name="extra" />
        </template>
    </div>

    <!-- Modals -->
    <ContentUpdaterModal
        v-if="form.updatingModpack.value"
        ref="contentUpdaterModal"
        :versions="form.updatingProjectVersions.value"
        :current-game-version="ctx.updaterModalProps.value.currentGameVersion"
        :current-loader="ctx.updaterModalProps.value.currentLoader"
        :current-version-id="ctx.updaterModalProps.value.currentVersionId"
        :is-app="ctx.isApp"
        project-type="modpack"
        :project-icon-url="ctx.updaterModalProps.value.projectIconUrl"
        :project-name="ctx.updaterModalProps.value.projectName"
        :loading="form.loadingVersions.value"
        :loading-changelog="form.loadingChangelog.value"
        @update="handleModpackUpdateRequest"
        @cancel="form.resetUpdateState()"
        @version-select="form.handleUpdaterVersionSelect"
        @version-hover="form.handleUpdaterVersionHover"
    />
    <ConfirmModpackUpdateModal
        ref="modpackUpdateModal"
        :downgrade="isUpdateDowngrade"
        :backup-tip="
            [ctx.modpack.value?.title, pendingUpdateVersion?.version_number]
                .filter(Boolean)
                .join(' ')
        "
        @confirm="handleModpackUpdateConfirm"
        @cancel="handleModpackUpdateCancel"
    />
    <ConfirmRepairModal
        ref="repairModal"
        :server="ctx.isServer"
        :symlink-target="ctx.symlinkTarget?.value ?? undefined"
        @repair="handleRepair"
    />
    <ConfirmReinstallModal
        ref="reinstallModal"
        :server="ctx.isServer"
        :backup-tip="ctx.modpack.value?.title"
        :symlink-target="ctx.symlinkTarget?.value ?? undefined"
        @reinstall="handleReinstall"
    />
    <ConfirmUnlinkModal
        ref="unlinkModal"
        :server="ctx.isServer"
        :backup-tip="ctx.modpack.value?.title"
        :action="handleUnlink"
    />

    <ConfirmLeaveModal ref="confirmLeaveModal" />
</template>
