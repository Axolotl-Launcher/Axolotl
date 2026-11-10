<template>
    <div class="space-y-6">
        <!-- Instance-specific: Icon upload -->
        <div v-if="ctx.flowType === 'instance'" class="flex items-center gap-4">
            <Avatar
                :src="ctx.instanceIconUrl.value ?? defaultInstanceIconUrl ?? undefined"
                size="5rem"
            />
            <div class="flex flex-col gap-2">
                <Button type="outlined" @click="triggerIconInput"
                    ><UploadIcon />
                    {{ formatMessage(messages.selectIcon) }}
                </Button>
                <Button type="outlined" :disabled="!ctx.instanceIcon.value" @click="removeIcon"
                    ><XIcon />
                    {{ formatMessage(messages.removeIcon) }}
                </Button>
            </div>
        </div>

        <!-- Instance-specific: Name field -->
        <div v-if="ctx.flowType === 'instance'" class="flex flex-col gap-2">
            <span class="font-semibold text-[var(--color-text-primary)]">{{
                formatMessage(messages.nameLabel)
            }}</span>
            <StyledInput
                v-model="ctx.instanceName.value"
                :placeholder="
                    ctx.autoInstanceName.value || formatMessage(messages.instanceNamePlaceholder)
                "
            />
        </div>

        <GameDirectoryPicker
            v-if="ctx.flowType === 'instance'"
            v-model="gameDirSelection"
            :directories="configuredMinecraftDirectories"
        />

        <!-- Game version -->
        <div class="flex flex-col gap-2">
            <span class="font-semibold text-[var(--color-text-primary)]">{{
                formatMessage(commonMessages.gameVersionLabel)
            }}</span>
            <div class="flex gap-2">
                <!-- Version type selector -->
                <Chips
                    v-model="selectedVersionType"
                    :items="versionTypeItems"
                    :format-label="formatVersionTypeLabel"
                />
            </div>
            <!-- Game version combobox -->
            <Combobox
                v-model="selectedGameVersion"
                :options="filteredGameVersionOptions"
                :disabled="gameVersionsLoading"
                :no-options-message="gameVersionNoOptionsMessage"
                :show-no-options-when-empty="gameVersionMetadataState !== 'ready'"
                searchable
                sync-with-selection
                :placeholder="gameVersionSelectorLabels.placeholder"
                :search-placeholder="gameVersionSelectorLabels.searchPlaceholder"
                @option-hover="handleGameVersionHover"
            />
        </div>

        <!-- Loader chips -->
        <div v-if="!hideLoaderChips" class="flex flex-col gap-2">
            <span class="font-semibold text-[var(--color-text-primary)]">{{
                ctx.flowType === 'instance'
                    ? formatMessage(messages.loaderLabel)
                    : formatMessage(messages.contentLoaderLabel)
            }}</span>
            <Chips
                v-model="selectedLoader"
                :items="effectiveLoaders"
                :format-label="localizedFormatLoaderLabel"
                :disabled-items="disabledLoaders"
                :disabled-tooltip="loaderDisabledTooltip"
            />
        </div>

        <!-- Loader version -->
        <template v-if="!hideLoaderVersion">
            <Collapsible :collapsed="!selectedLoader || !selectedGameVersion" overflow-visible>
                <div class="flex flex-col gap-2">
                    <div class="flex items-center gap-2">
                        <span class="font-semibold text-[var(--color-text-primary)]">{{
                            isPaperLike
                                ? formatMessage(messages.buildNumberLabel)
                                : formatMessage(messages.loaderVersionLabel)
                        }}</span>
                        <span
                            v-if="loaderVersionSummary === 'loading'"
                            class="text-sm text-[var(--color-text-tertiary)]"
                        >
                            {{ formatMessage(commonMessages.loadingLabel) }}
                        </span>
                        <span
                            v-else-if="
                                !isPaperLike &&
                                loaderVersionType !== 'other' &&
                                loaderVersionSummary === 'selected'
                            "
                            class="text-sm text-[var(--color-text-tertiary)]"
                        >
                            {{
                                formatMessage(messages.willInstallLoaderVersion, {
                                    loader: formatLoaderLabel(selectedLoader || '', formatMessage),
                                    version: selectedLoaderVersion,
                                })
                            }}
                        </span>
                    </div>
                    <Chips
                        v-if="!isPaperLike"
                        v-model="loaderVersionType"
                        :items="loaderVersionTypeItems"
                        :disabled-items="loaderVersionTypeDisabledItems"
                        :disabled-tooltip="formatMessage(messages.noSuchVersionsAvailable)"
                        :format-label="formatLoaderVersionTypeLabel"
                    />
                    <div v-if="isPaperLike || loaderVersionType === 'other'">
                        <Combobox
                            v-model="selectedLoaderVersion"
                            :options="loaderVersionOptions"
                            :disabled="loaderVersionsLoading"
                            :no-options-message="
                                loaderVersionsLoading
                                    ? formatMessage(commonMessages.loadingLabel)
                                    : formatMessage(messages.noVersionsAvailable)
                            "
                            searchable
                            sync-with-selection
                            :placeholder="loaderVersionSelectorLabels.placeholder"
                            :search-placeholder="loaderVersionSelectorLabels.searchPlaceholder"
                        >
                            <!-- When not Paper, this scoped slot is omitted and Combobox uses default option markup. -->
                            <template
                                v-if="selectedLoader === 'paper'"
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
                                            :channel="paperBuildChannelTag(String(item.value))"
                                        />
                                    </div>
                                </div>
                            </template>
                            <template
                                v-if="selectedLoader === 'paper'"
                                #search-selection-affix="{ option }"
                            >
                                <PaperChannelBadge
                                    affix
                                    :channel="
                                        option ? paperBuildChannelTag(String(option.value)) : null
                                    "
                                />
                            </template>
                        </Combobox>
                    </div>
                </div>
            </Collapsible>
        </template>

        <div
            v-if="ctx.flowType === 'instance' && adjunctOptions.length"
            class="flex flex-col gap-2"
        >
            <span class="font-semibold text-[var(--color-text-primary)]">{{
                formatMessage(messages.adjunctLabel)
            }}</span>
            <div class="flex flex-wrap gap-x-6 gap-y-3">
                <div
                    v-for="adjunct in adjunctOptions"
                    :key="adjunct"
                    class="flex min-w-40 max-w-64 flex-col gap-1"
                >
                    <Checkbox
                        :model-value="isAdjunctSelected(adjunct)"
                        :disabled="adjunctAvailability[adjunct].disabled"
                        :label="formatLoaderLabel(adjunct, formatMessage)"
                        @update:model-value="toggleAdjunct(adjunct)"
                    />
                    <span
                        v-if="adjunctAvailability[adjunct].reason"
                        class="text-xs leading-normal text-[var(--color-text-tertiary)]"
                    >
                        {{ adjunctAvailability[adjunct].reason }}
                    </span>
                </div>
            </div>
            <span class="text-sm text-[var(--color-text-tertiary)]">{{
                formatMessage(messages.adjunctHint)
            }}</span>
        </div>
    </div>
</template>

<script setup lang="ts">
import type { Paper } from '@modrinth/api-client'
import { UploadIcon, XIcon } from '@modrinth/assets'
import { commonMessages, defineMessages, useVIntl } from '@modrinth/ui'
import { computed, onMounted, ref, watch } from 'vue'

import { useDebugLogger } from '#ui/composables/debug-logger'
import { BUILTIN_GAME_DIRECTORY, loadMinecraftDirectories } from '#ui/utils/minecraft-directories'

import { injectFilePicker, injectModrinthClient, injectTags } from '../../../../providers'
import Avatar from '../../../base/Avatar.vue'
import Button from '../../../base/buttons/Button.vue'
import Checkbox from '../../../base/Checkbox.vue'
import Chips from '../../../base/Chips.vue'
import Collapsible from '../../../base/Collapsible.vue'
import Combobox, { type ComboboxOption } from '../../../base/Combobox.vue'
import PaperChannelBadge from '../../../base/PaperChannelBadge.vue'
import StyledInput from '../../../base/StyledInput.vue'
import type { AdjunctLoader, LoaderVersionEntry, LoaderVersionType } from '../creation-flow-context'
import { injectCreationFlowContext } from '../creation-flow-context'
import {
    createLatestRequestGuard,
    type GameVersionMetadataState,
    gameVersionSelectorText,
    isLoaderSupportStateDisabled,
    loaderMetadataCacheKey,
    type LoaderMetadataStatus,
    type LoaderSupportState,
    loaderSupportState,
    loaderVersionSelectorText,
    loaderVersionsForGameVersion,
    loaderVersionSummaryState,
    preserveOrSelectGameVersion,
} from '../loader-metadata'
import { formatLoaderLabel, type GameVersionType, isVersionTypeMatch } from '../shared'
import GameDirectoryPicker from './GameDirectoryPicker.vue'

const localizedFormatLoaderLabel = (item: string) => formatLoaderLabel(item, formatMessage)

const debug = useDebugLogger('CustomSetupStage')
const client = injectModrinthClient()
const ctx = injectCreationFlowContext()
const { formatMessage } = useVIntl()
const {
    selectedLoader,
    selectedGameVersion,
    loaderVersionType,
    selectedLoaderVersion,
    hideLoaderChips,
    hideLoaderVersion,
} = ctx

const messages = defineMessages({
    selectIcon: {
        id: 'creation-flow.modal.custom-setup.icon.select',
        defaultMessage: 'Select icon',
    },
    removeIcon: {
        id: 'creation-flow.modal.custom-setup.icon.remove',
        defaultMessage: 'Remove icon',
    },
    nameLabel: {
        id: 'creation-flow.modal.custom-setup.name.label',
        defaultMessage: 'Name',
    },
    instanceNamePlaceholder: {
        id: 'creation-flow.modal.custom-setup.name.placeholder',
        defaultMessage: 'Enter instance name',
    },
    loaderLabel: {
        id: 'creation-flow.modal.custom-setup.loader.label',
        defaultMessage: 'Mod Loader',
    },
    adjunctLabel: {
        id: 'creation-flow.modal.custom-setup.adjunct.label',
        defaultMessage: 'Additional loaders',
    },
    adjunctHint: {
        id: 'creation-flow.modal.custom-setup.adjunct.hint',
        defaultMessage: 'A compatible version will be selected before downloading.',
    },
    adjunctSelectGameVersion: {
        id: 'creation-flow.modal.custom-setup.adjunct.select-game-version',
        defaultMessage: 'Select a game version to check compatibility.',
    },
    adjunctCheckingCompatibility: {
        id: 'creation-flow.modal.custom-setup.adjunct.checking-compatibility',
        defaultMessage: 'Checking compatibility...',
    },
    adjunctPrimaryUnsupported: {
        id: 'creation-flow.modal.custom-setup.adjunct.primary-unsupported',
        defaultMessage: 'The selected mod loader does not support this game version.',
    },
    adjunctUnsupported: {
        id: 'creation-flow.modal.custom-setup.adjunct.unsupported',
        defaultMessage: 'No compatible {loader} version is available for this game version.',
    },
    adjunctCompatibilityLoadFailed: {
        id: 'creation-flow.modal.custom-setup.adjunct.compatibility-load-failed',
        defaultMessage: 'Failed to verify {loader} compatibility.',
    },
    adjunctOptiFabricUnsupported: {
        id: 'creation-flow.modal.custom-setup.adjunct.optifabric-unsupported',
        defaultMessage: 'OptiFine requires OptiFabric, but no compatible version is available.',
    },
    adjunctOptiFabricLoadFailed: {
        id: 'creation-flow.modal.custom-setup.adjunct.optifabric-load-failed',
        defaultMessage: 'Failed to verify OptiFabric compatibility.',
    },
    contentLoaderLabel: {
        id: 'creation-flow.modal.custom-setup.content-loader.label',
        defaultMessage: 'Content loader',
    },
    noVersionsAvailable: {
        id: 'creation-flow.modal.custom-setup.options.no-versions-available',
        defaultMessage: 'No versions available',
    },
    noGameVersionsForLoader: {
        id: 'creation-flow.modal.custom-setup.game-version.no-supported-versions',
        defaultMessage: 'No game versions support this loader',
    },
    gameVersionsLoadFailed: {
        id: 'creation-flow.modal.custom-setup.game-version.load-failed',
        defaultMessage: 'Failed to load game versions',
    },
    selectGameVersion: {
        id: 'creation-flow.modal.custom-setup.game-version.placeholder',
        defaultMessage: 'Select game version',
    },
    searchGameVersion: {
        id: 'creation-flow.modal.custom-setup.game-version.search-placeholder',
        defaultMessage: 'Search game version...',
    },
    buildNumberLabel: {
        id: 'creation-flow.modal.custom-setup.build-number.label',
        defaultMessage: 'Build number',
    },
    loaderVersionLabel: {
        id: 'creation-flow.modal.custom-setup.loader-version.label',
        defaultMessage: 'Mod Loader version',
    },
    willInstallLoaderVersion: {
        id: 'creation-flow.modal.custom-setup.will-install-loader-version',
        defaultMessage: 'Will install: {loader} {version}',
    },
    selectBuildNumber: {
        id: 'creation-flow.modal.custom-setup.build-number.placeholder',
        defaultMessage: 'Select build number',
    },
    selectLoaderVersion: {
        id: 'creation-flow.modal.custom-setup.loader-version.placeholder',
        defaultMessage: 'Select loader version',
    },
    searchBuildNumber: {
        id: 'creation-flow.modal.custom-setup.build-number.search-placeholder',
        defaultMessage: 'Search build number...',
    },
    searchLoaderVersion: {
        id: 'creation-flow.modal.custom-setup.loader-version.search-placeholder',
        defaultMessage: 'Search loader version...',
    },
    stableLoaderVersionType: {
        id: 'creation-flow.modal.custom-setup.loader-version-type.stable',
        defaultMessage: 'Stable',
    },
    latestLoaderVersionType: {
        id: 'creation-flow.modal.custom-setup.loader-version-type.latest',
        defaultMessage: 'Latest',
    },
    otherLoaderVersionType: {
        id: 'creation-flow.modal.custom-setup.loader-version-type.other',
        defaultMessage: 'Custom',
    },
    loaderUnsupportedTooltip: {
        id: 'creation-flow.modal.custom-setup.loader.unsupported-tooltip',
        defaultMessage: 'This loader does not support the selected game version',
    },
    loaderCompatibilityLoadFailed: {
        id: 'creation-flow.modal.custom-setup.loader.compatibility-load-failed',
        defaultMessage: 'Failed to verify loader compatibility',
    },
    noSuchVersionsAvailable: {
        id: 'creation-flow.modal.custom-setup.loader.no-such-versions-available',
        defaultMessage: 'No such versions available',
    },
    releaseVersionType: {
        id: 'creation-flow.modal.custom-setup.game-version-type.release',
        defaultMessage: 'Release',
    },
    snapshotVersionType: {
        id: 'creation-flow.modal.custom-setup.game-version-type.snapshot',
        defaultMessage: 'Snapshot',
    },
    alphaVersionType: {
        id: 'creation-flow.modal.custom-setup.game-version-type.alpha',
        defaultMessage: 'April Fools',
    },
    ancientVersionType: {
        id: 'creation-flow.modal.custom-setup.game-version-type.ancient',
        defaultMessage: 'Ancient',
    },
})

function formatLoaderVersionTypeLabel(type: LoaderVersionType): string {
    switch (type) {
        case 'stable':
            return formatMessage(messages.stableLoaderVersionType)
        case 'latest':
            return formatMessage(messages.latestLoaderVersionType)
        case 'other':
            return formatMessage(messages.otherLoaderVersionType)
    }
}

// Version type selection
const selectedVersionType = ref<GameVersionType>('release')

const versionTypeItems: GameVersionType[] = ['release', 'snapshot', 'alpha', 'ancient']

function formatVersionTypeLabel(type: GameVersionType): string {
    switch (type) {
        case 'release':
            return formatMessage(messages.releaseVersionType)
        case 'snapshot':
            return formatMessage(messages.snapshotVersionType)
        case 'alpha':
            return formatMessage(messages.alphaVersionType)
        case 'ancient':
            return formatMessage(messages.ancientVersionType)
    }
}

// For instance flow, prepend 'vanilla' to available loaders.
// For server flows, vanilla is a separate option in the setup type stage, so exclude it here.
const effectiveLoaders = computed(() => {
    if (ctx.flowType === 'instance') {
        return ['vanilla', ...ctx.availableLoaders.filter((l) => l !== 'vanilla')]
    }
    return ctx.availableLoaders
})

const adjunctOptions = computed(() => {
    if (ctx.flowType !== 'instance') return [] satisfies AdjunctLoader[]

    switch (selectedLoader.value) {
        case 'forge':
            return ['lite_loader', 'optifine'] satisfies AdjunctLoader[]
        case 'neoforge':
            return ['optifine'] satisfies AdjunctLoader[]
        case 'fabric':
            return ['optifine'] satisfies AdjunctLoader[]
        case 'legacy_fabric':
            return ['optifine'] satisfies AdjunctLoader[]
        default:
            return [] satisfies AdjunctLoader[]
    }
})

interface AdjunctAvailability {
    disabled: boolean
    reason?: string
}

const optiFabricCompatibility = ref<Record<string, boolean>>({})
const optiFabricMetadataStatus = ref<Record<string, LoaderMetadataStatus>>({})

function loaderStateAvailability(
    state: LoaderSupportState,
    unsupportedReason: string,
    loadFailedReason: string,
): AdjunctAvailability {
    if (state === 'supported') return { disabled: false }
    if (state === 'unsupported') return { disabled: true, reason: unsupportedReason }
    if (state === 'error') return { disabled: true, reason: loadFailedReason }
    return {
        disabled: true,
        reason: formatMessage(messages.adjunctCheckingCompatibility),
    }
}

const optiFabricCompatibilityState = computed<LoaderSupportState>(() => {
    const gameVersion = selectedGameVersion.value
    if (!gameVersion) return 'unknown'
    const status = optiFabricMetadataStatus.value[gameVersion] ?? 'unknown'
    if (status !== 'success') return status
    return optiFabricCompatibility.value[gameVersion] ? 'supported' : 'unsupported'
})

function resolveAdjunctAvailability(adjunct: AdjunctLoader): AdjunctAvailability {
    const gameVersion = selectedGameVersion.value
    if (!gameVersion) {
        return {
            disabled: true,
            reason: formatMessage(messages.adjunctSelectGameVersion),
        }
    }

    const primary = selectedLoader.value
    if (!primary) {
        return {
            disabled: true,
            reason: formatMessage(messages.adjunctCheckingCompatibility),
        }
    }

    const primaryAvailability = loaderStateAvailability(
        loaderCompatibilityState(gameVersion, primary),
        formatMessage(messages.adjunctPrimaryUnsupported),
        formatMessage(messages.loaderCompatibilityLoadFailed),
    )
    if (primaryAvailability.disabled) return primaryAvailability

    const loaderLabel = formatLoaderLabel(adjunct, formatMessage)
    const adjunctAvailability = loaderStateAvailability(
        loaderCompatibilityState(gameVersion, adjunct),
        formatMessage(messages.adjunctUnsupported, { loader: loaderLabel }),
        formatMessage(messages.adjunctCompatibilityLoadFailed, { loader: loaderLabel }),
    )
    if (adjunctAvailability.disabled) return adjunctAvailability

    if (adjunct === 'optifine' && (primary === 'fabric' || primary === 'legacy_fabric')) {
        return loaderStateAvailability(
            optiFabricCompatibilityState.value,
            formatMessage(messages.adjunctOptiFabricUnsupported),
            formatMessage(messages.adjunctOptiFabricLoadFailed),
        )
    }

    return { disabled: false }
}

const adjunctAvailability = computed<Record<AdjunctLoader, AdjunctAvailability>>(() => ({
    optifine: resolveAdjunctAvailability('optifine'),
    lite_loader: resolveAdjunctAvailability('lite_loader'),
}))

function isAdjunctSelected(adjunct: AdjunctLoader) {
    return ctx.selectedAdjuncts.value.includes(adjunct)
}

function toggleAdjunct(adjunct: AdjunctLoader) {
    if (adjunctAvailability.value[adjunct].disabled) return
    ctx.selectedAdjuncts.value = isAdjunctSelected(adjunct)
        ? ctx.selectedAdjuncts.value.filter((item) => item !== adjunct)
        : [...ctx.selectedAdjuncts.value, adjunct]
}

watch(
    [adjunctOptions, adjunctAvailability],
    ([options, availability]) => {
        ctx.selectedAdjuncts.value = ctx.selectedAdjuncts.value.filter(
            (item) => options.includes(item) && !availability[item].disabled,
        )
    },
    { immediate: true },
)

function loaderCompatibilityState(gameVersion: string, loader: string): LoaderSupportState {
    if (loader === 'vanilla') return 'supported'

    if (loader === 'paper' || loader === 'purpur') {
        const status = ctx.loaderMetadataStatus.value[loader] ?? 'unknown'
        if (status !== 'success') return status
        const supportedVersions =
            loader === 'paper'
                ? ctx.paperSupportedVersions.value
                : ctx.purpurSupportedVersions.value
        return supportedVersions?.has(gameVersion) ? 'supported' : 'unsupported'
    }

    const apiLoader = toApiLoaderName(loader)
    const cacheKey = loaderMetadataCacheKey(apiLoader, gameVersion)
    return loaderSupportState(
        ctx.loaderMetadataStatus.value[cacheKey] ?? 'unknown',
        ctx.loaderVersionsCache.value[cacheKey],
        gameVersion,
    )
}

const disabledLoaders = computed(() => {
    const gameVersion = selectedGameVersion.value
    if (!gameVersion) return []

    return effectiveLoaders.value.filter((loader) =>
        isLoaderSupportStateDisabled(loaderCompatibilityState(gameVersion, loader)),
    )
})

// Only confirmed incompatibility changes the current selection. Pending metadata merely disables it.
const unsupportedLoaders = computed(() => {
    const gameVersion = selectedGameVersion.value
    if (!gameVersion) return []

    return effectiveLoaders.value.filter(
        (loader) => loaderCompatibilityState(gameVersion, loader) === 'unsupported',
    )
})

function loaderDisabledTooltip(loader: string): string | undefined {
    const gameVersion = selectedGameVersion.value
    if (!gameVersion) return undefined

    const state = loaderCompatibilityState(gameVersion, loader)
    if (state === 'unknown' || state === 'loading') {
        return formatMessage(commonMessages.loadingLabel)
    }
    if (state === 'unsupported') return formatMessage(messages.loaderUnsupportedTooltip)
    if (state === 'error') return formatMessage(messages.loaderCompatibilityLoadFailed)
    return undefined
}

watch(
    selectedGameVersion,
    (gameVersion) => {
        if (gameVersion) void ctx.prefetchLoaderMetadata(gameVersion)
    },
    { immediate: true },
)

watch(
    [adjunctOptions, () => selectedGameVersion.value],
    ([options, gameVersion]) => {
        if (!gameVersion) return
        void Promise.allSettled(
            options.map((adjunct) => ctx.fetchLoaderMetadata(adjunct, gameVersion)),
        )
    },
    { immediate: true },
)

watch(
    [() => selectedLoader.value, () => selectedGameVersion.value],
    async ([loader, gameVersion]) => {
        if (ctx.flowType !== 'instance') return
        if ((loader !== 'fabric' && loader !== 'legacy_fabric') || !gameVersion) return
        const currentStatus = optiFabricMetadataStatus.value[gameVersion]
        if (currentStatus === 'loading' || currentStatus === 'success') {
            return
        }

        optiFabricMetadataStatus.value[gameVersion] = 'loading'
        try {
            optiFabricCompatibility.value[gameVersion] =
                await ctx.hasCompatibleOptiFabric(gameVersion)
            optiFabricMetadataStatus.value[gameVersion] = 'success'
        } catch (error) {
            optiFabricMetadataStatus.value[gameVersion] = 'error'
            debug('Failed to load OptiFabric compatibility metadata', error)
        }
    },
    { immediate: true },
)

watch([selectedGameVersion, unsupportedLoaders], () => {
    if (selectedLoader.value && unsupportedLoaders.value.includes(selectedLoader.value)) {
        const fallback = effectiveLoaders.value.includes('vanilla')
            ? 'vanilla'
            : effectiveLoaders.value.find((loader) => !unsupportedLoaders.value.includes(loader))
        selectedLoader.value = fallback ?? null
    }
})

// Pre-select loader and game version from initial values
onMounted(() => {
    debug(
        'mounted, initialLoader:',
        ctx.initialLoader,
        'initialGameVersion:',
        ctx.initialGameVersion,
    )
    if (!selectedLoader.value) {
        if (ctx.initialLoader) {
            selectedLoader.value = ctx.initialLoader
        } else {
            selectedLoader.value = 'vanilla'
        }
    }
    if (ctx.initialGameVersion && !selectedGameVersion.value) {
        selectedGameVersion.value = ctx.initialGameVersion
    }
    debug('after init:', { loader: selectedLoader.value, gameVersion: selectedGameVersion.value })
})

const tags = injectTags()

const loaderVersionTypeItems: LoaderVersionType[] = ['stable', 'latest', 'other']

const loaderVersionTypeDisabledItems = computed<LoaderVersionType[]>(() => {
    const noStableVersions = !loaderVersionsData.value.some((v: LoaderVersionEntry) => v.stable)
    return noStableVersions ? ['stable'] : []
})

const isPaperLike = computed(
    () => selectedLoader.value === 'paper' || selectedLoader.value === 'purpur',
)

// Icon upload handling
const filePicker = injectFilePicker()
const defaultInstanceIconUrl = computed(() =>
    selectedLoader.value
        ? (filePicker.getLoaderInstanceIconUrl?.(selectedLoader.value) ?? null)
        : null,
)

async function triggerIconInput() {
    const picked = await (filePicker.pickInstanceIcon?.() ?? filePicker.pickImage())
    if (picked) {
        ctx.instanceIcon.value = picked.file
        ctx.instanceIconUrl.value = picked.previewUrl
        ctx.instanceIconPath.value = picked.path ?? null
    }
}

function removeIcon() {
    ctx.instanceIcon.value = null
    ctx.instanceIconUrl.value = null
    ctx.instanceIconPath.value = null
}

const configuredMinecraftDirectories = loadMinecraftDirectories()
const configuredMinecraftDirectoryModes = new Map(
    configuredMinecraftDirectories.map((entry) => [entry.path, entry.mode]),
)

const gameDirSelection = computed<string>({
    get: () =>
        ctx.gameDirOverrideMode.value === 'builtin'
            ? BUILTIN_GAME_DIRECTORY
            : (ctx.gameDirOverride.value ?? BUILTIN_GAME_DIRECTORY),
    set: (selection) => {
        if (selection === BUILTIN_GAME_DIRECTORY) {
            ctx.gameDirOverrideMode.value = 'builtin'
            ctx.gameDirOverride.value = null
            return
        }
        ctx.gameDirOverrideMode.value =
            configuredMinecraftDirectoryModes.get(selection) === 'shared'
                ? 'not-isolated'
                : 'isolated'
        ctx.gameDirOverride.value = selection
    },
})

const loaderVersionsLoading = ref(false)
const loaderVersionsData = ref<LoaderVersionEntry[]>([])
const loaderVersionSummary = computed(() =>
    loaderVersionSummaryState(loaderVersionsLoading.value, selectedLoaderVersion.value),
)

const loaderVersionSelectorLabels = computed(() => {
    const loading = formatMessage(commonMessages.loadingLabel)
    const empty = formatMessage(messages.noVersionsAvailable)
    const placeholder = isPaperLike.value
        ? formatMessage(messages.selectBuildNumber)
        : formatMessage(messages.selectLoaderVersion)
    const searchPlaceholder = isPaperLike.value
        ? formatMessage(messages.searchBuildNumber)
        : formatMessage(messages.searchLoaderVersion)
    const loader = selectedLoader.value
    const gameVersion = selectedGameVersion.value
    let resolvedEmpty = false

    if (loader && gameVersion && !isPaperLike.value && loader !== 'vanilla') {
        const apiLoader = toApiLoaderName(loader)
        const cacheKey = loaderMetadataCacheKey(apiLoader, gameVersion)
        resolvedEmpty =
            loaderSupportState(
                ctx.loaderMetadataStatus.value[cacheKey] ?? 'unknown',
                ctx.loaderVersionsCache.value[cacheKey],
                gameVersion,
            ) === 'unsupported'
    }

    return loaderVersionSelectorText(loaderVersionsLoading.value, resolvedEmpty, {
        loading,
        empty,
        placeholder,
        searchPlaceholder,
    })
})

// Paper/Purpur build caches
const paperVersions = ref<Record<string, Paper.Versions.v3.Build[]>>({})
const purpurVersions = ref<Record<string, string[]>>({})

function toApiLoaderName(loader: string): string {
    return loader === 'neoforge' ? 'neo' : loader
}

function isGameVersionListedByLoader(gameVersion: string, loader: string): boolean {
    if (loader === 'vanilla') return true
    if (loader === 'paper') return ctx.paperSupportedVersions.value?.has(gameVersion) ?? false
    if (loader === 'purpur') return ctx.purpurSupportedVersions.value?.has(gameVersion) ?? false

    const manifest = ctx.loaderVersionsCache.value[toApiLoaderName(loader)]
    if (!manifest) return false

    const hasPlaceholder = manifest.gameVersions.some((x) => x.id === '${modrinth.gameVersion}')
    if (hasPlaceholder) return true

    return manifest.gameVersions.some(
        (x) => x.id === gameVersion && (x.loaders.length > 0 || !!x.versionGroup),
    )
}

const gameVersionsLoading = computed(() => {
    const loader = selectedLoader.value
    if (!loader || loader === 'vanilla') return false
    const status = ctx.loaderMetadataStatus.value[toApiLoaderName(loader)] ?? 'unknown'
    return status === 'unknown' || status === 'loading'
})

// Game versions from tags provider, filtered by loader support
const gameVersionOptions = computed<Array<ComboboxOption<string> & { versionType?: string }>>(
    () => {
        const versions = tags.gameVersions.value

        if (selectedLoader.value && selectedLoader.value !== 'vanilla') {
            const loader = selectedLoader.value
            return versions
                .filter((v) => isGameVersionListedByLoader(v.version, loader))
                .map((v) => ({ value: v.version, label: v.version, versionType: v.version_type }))
        }

        return versions.map((v) => ({
            value: v.version,
            label: v.version,
            versionType: v.version_type,
        }))
    },
)

// Filtered game versions based on selected version type
const filteredGameVersionOptions = computed<ComboboxOption<string>[]>(() => {
    const allOptions = gameVersionOptions.value
    return allOptions.filter((opt) => {
        if (!opt.versionType) return false
        return isVersionTypeMatch(opt.versionType, String(opt.value), selectedVersionType.value)
    })
})

const gameVersionMetadataState = computed<GameVersionMetadataState>(() => {
    const loader = selectedLoader.value
    if (!loader || loader === 'vanilla') return 'ready'

    const status = ctx.loaderMetadataStatus.value[toApiLoaderName(loader)] ?? 'unknown'
    if (status === 'unknown' || status === 'loading') return 'loading'
    if (status === 'error') return 'error'
    return filteredGameVersionOptions.value.length > 0 ? 'ready' : 'empty'
})

const gameVersionSelectorLabels = computed(() =>
    gameVersionSelectorText(gameVersionMetadataState.value, {
        loading: formatMessage(commonMessages.loadingLabel),
        empty: formatMessage(messages.noGameVersionsForLoader),
        error: formatMessage(messages.gameVersionsLoadFailed),
        placeholder: formatMessage(messages.selectGameVersion),
        searchPlaceholder: formatMessage(messages.searchGameVersion),
    }),
)

const gameVersionNoOptionsMessage = computed(() => {
    if (gameVersionMetadataState.value === 'loading') {
        return formatMessage(commonMessages.loadingLabel)
    }
    if (gameVersionMetadataState.value === 'empty') {
        return formatMessage(messages.noGameVersionsForLoader)
    }
    if (gameVersionMetadataState.value === 'error') {
        return formatMessage(messages.gameVersionsLoadFailed)
    }
    return formatMessage(messages.noVersionsAvailable)
})

// Select an initial game version without replacing an explicit user selection.
watch(
    gameVersionOptions,
    () => {
        const options = filteredGameVersionOptions.value
        selectedGameVersion.value = preserveOrSelectGameVersion(
            selectedGameVersion.value,
            options.map((option) => String(option.value)),
        )
    },
    { immediate: true },
)

// Auto-select latest game version when version type changes
watch(
    selectedVersionType,
    () => {
        const options = filteredGameVersionOptions.value
        if (options.length > 0) {
            selectedGameVersion.value = options[0].value
        } else {
            selectedGameVersion.value = null
        }
    },
    { immediate: true },
)

async function fetchLoaderManifest(loader: string, gameVersion: string) {
    const apiLoader = toApiLoaderName(loader)
    const cacheKey = loaderMetadataCacheKey(apiLoader, gameVersion)
    debug(
        'fetchLoaderManifest:',
        loader,
        'apiLoader:',
        apiLoader,
        'cached:',
        !!ctx.loaderVersionsCache.value[cacheKey],
    )
    await ctx.fetchLoaderMetadata(loader, gameVersion)
}

async function fetchLoaderMetadata(loader?: string | null) {
    await ctx.fetchLoaderMetadata(loader)
}

function paperBuildChannelTag(buildId: string): 'ALPHA' | 'BETA' | null {
    const gv = selectedGameVersion.value
    if (!gv || selectedLoader.value !== 'paper') return null
    const b = paperVersions.value[gv]?.find((x) => String(x.id) === buildId)
    if (!b) return null
    const u = String(b.channel).toUpperCase()
    if (u === 'ALPHA' || u === 'BETA') return u
    return null
}

async function fetchPaperVersions(mcVersion: string) {
    if (paperVersions.value[mcVersion]) return
    try {
        const data = await client.paper.versions_v3.getBuilds(mcVersion)
        paperVersions.value[mcVersion] = data.builds.toSorted((a, b) => b.id - a.id)
    } catch {
        paperVersions.value[mcVersion] = []
    }
}

function handleGameVersionHover(option: ComboboxOption<string | null>) {
    const v = option.value
    if (v == null || v === '') return
    if (selectedLoader.value === 'paper') void fetchPaperVersions(v)
    else if (selectedLoader.value === 'purpur') void fetchPurpurVersions(v)
}

async function fetchPurpurVersions(mcVersion: string) {
    if (purpurVersions.value[mcVersion]) return
    try {
        const data = await client.purpur.versions_v2.getBuilds(mcVersion)
        purpurVersions.value[mcVersion] = data.builds.all.sort((a, b) => parseInt(b) - parseInt(a))
    } catch {
        purpurVersions.value[mcVersion] = []
    }
}

function getLoaderVersionsForGameVersion(
    loader: string,
    gameVersion: string,
): LoaderVersionEntry[] {
    const apiLoader = toApiLoaderName(loader)
    const cacheKey = loaderMetadataCacheKey(apiLoader, gameVersion)
    const manifest = ctx.loaderVersionsCache.value[cacheKey]
    debug('getLoaderVersionsForGameVersion:', {
        loader,
        apiLoader,
        cacheKey,
        gameVersion,
        hasManifest: !!manifest,
        manifestLength: manifest?.gameVersions.length,
    })
    const loaders = loaderVersionsForGameVersion(manifest, gameVersion)
    debug('getLoaderVersionsForGameVersion: result', gameVersion, loaders.length + ' loaders')
    return loaders
}

// Fetch version data when loader changes so game versions can be filtered
watch(
    () => selectedLoader.value,
    async (loader) => {
        await fetchLoaderMetadata(loader)
    },
    { immediate: true },
)

// Watch loader + game version to resolve loader versions
const loaderVersionRequest = createLatestRequestGuard()
watch(
    [() => selectedLoader.value, () => selectedGameVersion.value],
    async ([loader, gameVersion]) => {
        const watchId = loaderVersionRequest.begin()
        debug('watch [loader, gameVersion] fired:', { loader, gameVersion, watchId })
        loaderVersionsLoading.value = false
        loaderVersionsData.value = []
        selectedLoaderVersion.value = null

        if (!loader || !gameVersion || loader === 'vanilla') return

        loaderVersionsLoading.value = true

        if (loader === 'paper') {
            await fetchPaperVersions(gameVersion)
            if (!loaderVersionRequest.isCurrent(watchId)) return
            loaderVersionsLoading.value = false
            const builds = paperVersions.value[gameVersion]
            if (builds?.length) {
                selectedLoaderVersion.value = `${builds[0].id}`
            }
            return
        }

        if (loader === 'purpur') {
            await fetchPurpurVersions(gameVersion)
            if (!loaderVersionRequest.isCurrent(watchId)) return
            loaderVersionsLoading.value = false
            const builds = purpurVersions.value[gameVersion]
            if (builds?.length) {
                selectedLoaderVersion.value = builds[0]
            }
            return
        }

        await fetchLoaderManifest(loader, gameVersion)
        if (!loaderVersionRequest.isCurrent(watchId)) {
            debug('watch [loader, gameVersion]: stale execution, skipping', {
                watchId,
            })
            return
        }
        loaderVersionsData.value = getLoaderVersionsForGameVersion(loader, gameVersion)
        debug(
            'watch [loader, gameVersion]: loaderVersionsData set, count:',
            loaderVersionsData.value.length,
        )
        loaderVersionsLoading.value = false

        // Auto-select based on loaderVersionType
        autoSelectLoaderVersion()
    },
)

watch(
    () => loaderVersionType.value,
    () => autoSelectLoaderVersion(),
)

function autoSelectLoaderVersion() {
    debug(
        'autoSelectLoaderVersion: type:',
        loaderVersionType.value,
        'dataCount:',
        loaderVersionsData.value.length,
        'stableCount:',
        loaderVersionsData.value.filter((v) => v.stable).length,
        'first:',
        loaderVersionsData.value[0]?.id,
    )
    if (
        loaderVersionType.value === 'stable' &&
        loaderVersionTypeDisabledItems.value.includes('stable')
    ) {
        debug("'stable' loader version type is disabled, switching to 'latest'...")
        loaderVersionType.value = 'latest'
    }
    if (loaderVersionType.value === 'stable') {
        const stable = loaderVersionsData.value.find((v) => v.stable)
        selectedLoaderVersion.value = stable?.id ?? loaderVersionsData.value[0]?.id ?? null
    } else if (loaderVersionType.value === 'latest') {
        selectedLoaderVersion.value = loaderVersionsData.value[0]?.id ?? null
    } else if (loaderVersionType.value === 'other' && !selectedLoaderVersion.value) {
        selectedLoaderVersion.value = loaderVersionsData.value[0]?.id ?? null
    }
    debug('autoSelectLoaderVersion: result:', selectedLoaderVersion.value)
}

const loaderVersionOptions = computed<ComboboxOption<string>[]>(() => {
    if (selectedLoader.value === 'paper' && selectedGameVersion.value) {
        const builds = paperVersions.value[selectedGameVersion.value] ?? []
        return builds.map((b) => ({
            value: `${b.id}`,
            label: `Build ${b.id}`,
        }))
    }

    if (selectedLoader.value === 'purpur' && selectedGameVersion.value) {
        const builds = purpurVersions.value[selectedGameVersion.value] ?? []
        return builds.map((b) => ({ value: b, label: `Build ${b}` }))
    }

    return loaderVersionsData.value.map((v) => ({
        value: v.id,
        label: v.stable ? `${v.id} (stable)` : v.id,
    }))
})
</script>
