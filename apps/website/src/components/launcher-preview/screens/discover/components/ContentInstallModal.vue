<script setup lang="ts">
import BoxIcon from '@modrinth/assets/icons/box.svg?component'
import CheckIcon from '@modrinth/assets/icons/check.svg?component'
import DownloadIcon from '@modrinth/assets/icons/download.svg?component'
import EyeIcon from '@modrinth/assets/icons/eye.svg?component'
import EyeOffIcon from '@modrinth/assets/icons/eye-off.svg?component'
import SearchIcon from '@modrinth/assets/icons/search.svg?component'
import TriangleAlertIcon from '@modrinth/assets/icons/triangle-alert.svg?component'
import UploadIcon from '@modrinth/assets/icons/upload.svg?component'
import XIcon from '@modrinth/assets/icons/x.svg?component'
import Avatar from '@modrinth/ui/src/components/base/Avatar.vue'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import Chips from '@modrinth/ui/src/components/base/Chips.vue'
import Combobox, { type ComboboxOption } from '@modrinth/ui/src/components/base/Combobox.vue'
import LoadingIndicator from '@modrinth/ui/src/components/base/LoadingIndicator.vue'
import StyledInput from '@modrinth/ui/src/components/base/StyledInput.vue'
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { commonMessages } from '@modrinth/ui/src/utils/common-messages.ts'
import { formatLoaderLabel } from '@modrinth/ui/src/utils/loaders.ts'
import { computed, ref, watch } from 'vue'

import PreviewModal from '../../../overlays/PreviewModal.vue'
import { injectContentInstall } from '../install'

/**
 * Port of "Install project" (packages/ui/src/layouts/shared/content-tab/components/modals/
 * ContentInstallModal.vue) wired the way App.vue wires it to the content install provider.
 * The browser's file picker stands in for the native icon dialog.
 */
const { formatMessage } = useVIntl()
const install = injectContentInstall()

const text = defineMessages({
    header: { id: 'instances.content-install.header', defaultMessage: 'Install project' },
    instanceType: {
        id: 'instances.content-install.instance-type',
        defaultMessage: 'Instance type',
    },
    existingTab: {
        id: 'instances.content-install.existing-tab',
        defaultMessage: 'Existing instance',
    },
    newTab: { id: 'instances.content-install.new-tab', defaultMessage: 'New instance' },
    searchPlaceholder: {
        id: 'instances.content-install.search-placeholder',
        defaultMessage: 'Search instance',
    },
    installedBadge: {
        id: 'instances.content-install.installed-badge',
        defaultMessage: 'Installed',
    },
    installButton: { id: 'instances.content-install.install-button', defaultMessage: 'Install' },
    incompatibleTooltip: {
        id: 'instances.content-install.incompatible-tooltip',
        defaultMessage:
            'This instance uses a different loader or game version than this project supports.',
    },
    selectIcon: { id: 'instances.content-install.select-icon', defaultMessage: 'Select icon' },
    removeIcon: { id: 'instances.content-install.remove-icon', defaultMessage: 'Remove icon' },
    nameLabel: { id: 'instances.content-install.name-label', defaultMessage: 'Name' },
    namePlaceholder: {
        id: 'instances.content-install.name-placeholder',
        defaultMessage: 'Enter instance name',
    },
    loaderLabel: { id: 'instances.content-install.loader-label', defaultMessage: 'Loader' },
    gameVersionPlaceholder: {
        id: 'instances.content-install.game-version-placeholder',
        defaultMessage: 'Select game version',
    },
    compatibleCount: {
        id: 'instances.content-install.compatible-count',
        defaultMessage: '{count} compatible {count, plural, one {instance} other {instances}}',
    },
    noInstances: {
        id: 'instances.content-install.no-instances',
        defaultMessage: 'No compatible instances found',
    },
})

const modal = ref<InstanceType<typeof PreviewModal>>()
const fileInput = ref<HTMLInputElement | null>(null)

type Tab = 'existing' | 'new'
const tabs = computed<Tab[]>(() =>
    install.compatibleLoaders.value.length > 0 ? ['existing', 'new'] : ['existing'],
)
const tab = ref<Tab>('existing')
const tabLabels: Record<Tab, () => string> = {
    existing: () => formatMessage(text.existingTab),
    new: () => formatMessage(text.newTab),
}
const formatTabLabel = (item: Tab) => tabLabels[item]()

const searchFilter = ref('')
const hideUninstallable = ref(true)

const filteredInstances = computed(() => {
    let list = install.instances.value
    if (hideUninstallable.value) list = list.filter((entry) => entry.compatible && !entry.installed)
    if (searchFilter.value) {
        const query = searchFilter.value.toLowerCase()
        list = list.filter((entry) => entry.name.toLowerCase().includes(query))
    }
    const score = (entry: { compatible: boolean; installed: boolean }) =>
        !entry.compatible ? 2 : entry.installed ? 1 : 0
    return list.slice().sort((a, b) => score(a) - score(b) || a.name.localeCompare(b.name))
})

const compatibleCount = computed(
    () => install.instances.value.filter((entry) => entry.compatible).length,
)

const instanceName = ref('')
const selectedLoader = ref<string | null>(null)
const selectedGameVersion = ref<string | null>(null)
const iconPreviewUrl = ref<string | null>(null)
const showSnapshots = ref(false)

const hasReleaseData = computed(() => install.releaseGameVersions.value.size > 0)

const gameVersionOptions = computed<ComboboxOption<string>[]>(() => {
    const versions =
        showSnapshots.value || !hasReleaseData.value
            ? install.gameVersions.value
            : install.gameVersions.value.filter((version) =>
                  install.releaseGameVersions.value.has(version),
              )
    return versions.map((version) => ({ value: version, label: version }))
})

function selectIcon() {
    fileInput.value?.click()
}

function onIconPicked(event: Event) {
    const file = (event.target as HTMLInputElement).files?.[0]
    if (file) iconPreviewUrl.value = URL.createObjectURL(file)
    ;(event.target as HTMLInputElement).value = ''
}

function removeIcon() {
    iconPreviewUrl.value = null
}

function resetState() {
    tab.value = install.defaultTab.value
    searchFilter.value = ''
    hideUninstallable.value = false
    instanceName.value = `New instance (${install.instances.value.length + 1})`
    iconPreviewUrl.value = null
    selectedLoader.value =
        install.preferredLoader.value ?? install.compatibleLoaders.value[0] ?? null
    const preferred = install.preferredGameVersion.value
    const isSnapshot =
        preferred && hasReleaseData.value && !install.releaseGameVersions.value.has(preferred)
    showSnapshots.value = !!isSnapshot
    const defaultVersion = hasReleaseData.value
        ? (install.gameVersions.value.find((version) =>
              install.releaseGameVersions.value.has(version),
          ) ??
          install.gameVersions.value[0] ??
          null)
        : (install.gameVersions.value[0] ?? null)
    selectedGameVersion.value = preferred ?? defaultVersion
}

watch(
    () => install.loading.value,
    (loading, wasLoading) => {
        if (wasLoading && !loading) tab.value = install.defaultTab.value
    },
)

function handleHide() {
    install.handleCancel()
}

function show() {
    resetState()
    modal.value?.show()
}

function hide() {
    modal.value?.hide()
}

function handleCreateAndInstall() {
    if (!instanceName.value || !selectedLoader.value || !selectedGameVersion.value) return
    void install.handleCreateAndInstall({
        name: instanceName.value,
        iconPath: null,
        iconPreviewUrl: iconPreviewUrl.value,
        loader: selectedLoader.value,
        gameVersion: selectedGameVersion.value,
    })
    hide()
}

defineExpose({ show, hide })
</script>

<template>
    <PreviewModal
        ref="modal"
        no-padding
        scrollable
        max-width="560px"
        width="560px"
        :on-hide="handleHide"
    >
        <template #title>
            <span class="text-2xl font-semibold text-[var(--color-text-primary)]">
                {{ formatMessage(text.header) }}
            </span>
        </template>

        <div class="flex flex-col gap-2.5 p-6">
            <span class="font-semibold text-[var(--color-text-primary)]">
                {{ formatMessage(text.instanceType) }}
            </span>
            <Chips
                v-model="tab"
                :items="tabs"
                :format-label="formatTabLabel"
                :never-empty="true"
                :capitalize="false"
            />
        </div>

        <div class="h-px bg-divider" />

        <div
            v-if="tab === 'existing'"
            class="flex flex-col gap-3 bg-surface-2 py-4"
            style="height: 400px; overflow-y: auto"
        >
            <div class="flex items-start gap-3 px-6">
                <StyledInput
                    v-model="searchFilter"
                    :icon="SearchIcon"
                    :placeholder="formatMessage(text.searchPlaceholder)"
                    class="flex-1"
                />
                <Button
                    v-tooltip="`${hideUninstallable ? 'Show' : 'Hide'} unavailable`"
                    type="outlined"
                    circular
                    icon-only
                    @click="hideUninstallable = !hideUninstallable"
                    ><EyeOffIcon v-if="hideUninstallable" />
                    <EyeIcon v-else />
                </Button>
            </div>

            <div v-if="install.loading.value" class="flex items-center justify-center py-12">
                <LoadingIndicator />
            </div>
            <div
                v-else-if="filteredInstances.length === 0"
                class="flex items-center justify-center py-12 text-[var(--color-text-tertiary)]"
            >
                {{ formatMessage(text.noInstances) }}
            </div>
            <div v-else class="flex flex-col gap-1">
                <div
                    v-for="inst in filteredInstances"
                    :key="inst.id"
                    class="flex items-center justify-between px-6 py-1.5"
                    :class="inst.installed ? 'opacity-60' : 'hover:bg-surface-3'"
                >
                    <button
                        v-tooltip="
                            !inst.compatible ? formatMessage(text.incompatibleTooltip) : undefined
                        "
                        class="flex min-w-0 cursor-pointer items-center gap-2.5 overflow-hidden border-0 bg-transparent p-0 text-left"
                        @click="install.handleNavigate(inst)"
                    >
                        <Avatar
                            :src="inst.iconUrl ?? undefined"
                            size="2rem"
                            rounded="md"
                            :class="{
                                '!rounded-none !border-0 !bg-transparent !shadow-none':
                                    inst.iconFrameless,
                            }"
                        />
                        <span
                            class="truncate font-semibold text-[var(--color-text-primary)] hover:underline"
                            >{{ inst.name }}</span
                        >
                    </button>
                    <Button v-if="inst.installed" disabled
                        ><CheckIcon />
                        {{ formatMessage(text.installedBadge) }}
                    </Button>
                    <Button
                        v-else
                        v-tooltip="
                            !inst.compatible ? formatMessage(text.incompatibleTooltip) : undefined
                        "
                        :type="inst.compatible ? 'base' : 'outlined'"
                        :color="inst.compatible ? undefined : 'orange'"
                        :disabled="inst.installing"
                        @click="install.handleInstallToInstance(inst)"
                    >
                        <TriangleAlertIcon v-if="!inst.compatible" />
                        {{
                            inst.installing
                                ? formatMessage(commonMessages.installingLabel)
                                : formatMessage(text.installButton)
                        }}
                    </Button>
                </div>
            </div>
        </div>

        <div v-else class="flex flex-col gap-6 p-6">
            <div class="flex items-center gap-4">
                <Avatar :src="iconPreviewUrl ?? undefined" size="5rem" rounded="2xl" />
                <div class="flex flex-col gap-2">
                    <Button type="outlined" @click="selectIcon"
                        ><UploadIcon />
                        {{ formatMessage(text.selectIcon) }}
                    </Button>
                    <Button type="outlined" :disabled="!iconPreviewUrl" @click="removeIcon"
                        ><XIcon />
                        {{ formatMessage(text.removeIcon) }}
                    </Button>
                </div>
                <input
                    ref="fileInput"
                    type="file"
                    accept="image/png,image/jpeg,image/gif,image/webp"
                    class="hidden"
                    @change="onIconPicked"
                />
            </div>

            <div class="flex flex-col gap-2.5">
                <span class="font-semibold text-[var(--color-text-primary)]">
                    {{ formatMessage(text.nameLabel) }}
                </span>
                <StyledInput
                    v-model="instanceName"
                    :placeholder="formatMessage(text.namePlaceholder)"
                />
            </div>

            <div class="flex flex-col gap-2.5">
                <span class="font-semibold text-[var(--color-text-primary)]">
                    {{ formatMessage(text.loaderLabel) }}
                </span>
                <Chips
                    v-model="selectedLoader"
                    :items="install.compatibleLoaders.value"
                    :format-label="formatLoaderLabel"
                    :never-empty="true"
                />
            </div>

            <div class="flex flex-col gap-2.5">
                <span class="font-semibold text-[var(--color-text-primary)]">
                    {{ formatMessage(commonMessages.gameVersionLabel) }}
                </span>
                <Combobox
                    v-model="selectedGameVersion"
                    :options="gameVersionOptions"
                    searchable
                    sync-with-selection
                    :placeholder="formatMessage(text.gameVersionPlaceholder)"
                >
                    <template v-if="hasReleaseData" #dropdown-footer>
                        <button
                            class="flex w-full cursor-pointer items-center justify-center gap-1.5 border-0 border-t border-solid border-surface-5 bg-transparent py-3 text-center text-sm font-semibold text-[var(--color-text-tertiary)] transition-colors hover:text-[var(--color-text-primary)]"
                            @mousedown.prevent
                            @click="showSnapshots = !showSnapshots"
                        >
                            <EyeOffIcon v-if="showSnapshots" class="size-4" />
                            <EyeIcon v-else class="size-4" />
                            {{
                                showSnapshots
                                    ? formatMessage(commonMessages.hideSnapshotsButton)
                                    : formatMessage(commonMessages.showAllVersionsButton)
                            }}
                        </button>
                    </template>
                </Combobox>
            </div>
        </div>

        <template #actions>
            <div v-if="tab === 'existing'" class="flex items-center justify-between px-4 pb-1 pt-5">
                <div class="flex items-center gap-1.5">
                    <BoxIcon class="size-5" />
                    <span>
                        {{ formatMessage(text.compatibleCount, { count: compatibleCount }) }}
                    </span>
                </div>
                <Button type="outlined" @click="modal?.hide()"
                    ><XIcon />
                    {{ formatMessage(commonMessages.cancelButton) }}
                </Button>
            </div>

            <div v-else class="flex items-center justify-end gap-2">
                <Button type="outlined" @click="modal?.hide()"
                    ><XIcon />
                    {{ formatMessage(commonMessages.cancelButton) }}
                </Button>
                <Button
                    type="colored"
                    color="brand"
                    :disabled="!instanceName"
                    @click="handleCreateAndInstall"
                    ><DownloadIcon />
                    {{ formatMessage(text.installButton) }}
                </Button>
            </div>
        </template>
    </PreviewModal>
</template>
