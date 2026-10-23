<script setup lang="ts">
import UploadIcon from '@modrinth/assets/icons/upload.svg?component'
import XIcon from '@modrinth/assets/icons/x.svg?component'
import Avatar from '@modrinth/ui/src/components/base/Avatar.vue'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import Checkbox from '@modrinth/ui/src/components/base/Checkbox.vue'
import Chips from '@modrinth/ui/src/components/base/Chips.vue'
import Collapsible from '@modrinth/ui/src/components/base/Collapsible.vue'
import Combobox, { type ComboboxOption } from '@modrinth/ui/src/components/base/Combobox.vue'
import RadioButtons from '@modrinth/ui/src/components/base/RadioButtons.vue'
import StyledInput from '@modrinth/ui/src/components/base/StyledInput.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { commonMessages } from '@modrinth/ui/src/utils/common-messages.ts'
import { formatLoaderLabel } from '@modrinth/ui/src/utils/loaders.ts'
import { computed, onMounted, ref, watch } from 'vue'

import { getLoaderInstanceIconUrl } from '../icons'
import { customSetupMessages as messages } from '../messages'
import {
    type GameVersionType,
    hasUnresolvedBuilds,
    isVersionTypeMatch,
    loaderMetadataCacheKey,
    type LoaderMetadataStatus,
    type LoaderSupportState,
    loaderSupportState,
    type LoaderVersionEntry,
    loaderVersionsForGameVersion,
    toApiLoaderName,
} from '../metadata'
import { type AdjunctLoader, type LoaderVersionType, useCreateArea } from '../state'

/**
 * Port of `creation-flow-modal/components/CustomSetupStage.vue` for the instance
 * flow: icon, name, game directory, game version (type chips + combobox), mod
 * loader chips, loader version (channel chips + custom combobox) and additional
 * loaders. Game versions are the tag list and loader builds the launcher-meta
 * manifests of the snapshot (`metadata.ts`).
 */
const localizedFormatLoaderLabel = (item: string) => formatLoaderLabel(item, formatMessage)

const area = useCreateArea()
const ctx = area.flow
const { formatMessage } = useVIntl()
const { selectedLoader, selectedGameVersion, loaderVersionType, selectedLoaderVersion } = ctx

type GameVersionMetadataState = 'loading' | 'ready' | 'empty' | 'error'

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

// The instance flow prepends 'vanilla' to the available loaders.
const effectiveLoaders = computed(() => [
    'vanilla',
    ...ctx.availableLoaders.filter((loader) => loader !== 'vanilla'),
])

const adjunctOptions = computed<AdjunctLoader[]>(() => {
    switch (selectedLoader.value) {
        case 'forge':
            return ['lite_loader', 'optifine']
        case 'neoforge':
        case 'fabric':
        case 'legacy_fabric':
            return ['optifine']
        default:
            return []
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
    return { disabled: true, reason: formatMessage(messages.adjunctCheckingCompatibility) }
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
        return { disabled: true, reason: formatMessage(messages.adjunctSelectGameVersion) }
    }
    const primary = selectedLoader.value
    if (!primary) {
        return { disabled: true, reason: formatMessage(messages.adjunctCheckingCompatibility) }
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
    const cacheKey = loaderMetadataCacheKey(toApiLoaderName(loader), gameVersion)
    return loaderSupportState(
        ctx.loaderMetadataStatus.value[cacheKey] ?? 'unknown',
        ctx.loaderVersionsCache.value[cacheKey],
        gameVersion,
    )
}

const disabledLoaders = computed(() => {
    const gameVersion = selectedGameVersion.value
    if (!gameVersion) return []
    return effectiveLoaders.value.filter(
        (loader) => loaderCompatibilityState(gameVersion, loader) !== 'supported',
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
    if (state === 'unknown' || state === 'loading')
        return formatMessage(commonMessages.loadingLabel)
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
        if ((loader !== 'fabric' && loader !== 'legacy_fabric') || !gameVersion) return
        const currentStatus = optiFabricMetadataStatus.value[gameVersion]
        if (currentStatus === 'loading' || currentStatus === 'success') return
        optiFabricMetadataStatus.value[gameVersion] = 'loading'
        try {
            optiFabricCompatibility.value[gameVersion] =
                await ctx.hasCompatibleOptiFabric(gameVersion)
            optiFabricMetadataStatus.value[gameVersion] = 'success'
        } catch {
            optiFabricMetadataStatus.value[gameVersion] = 'error'
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

// Pre-select the loader (the app's flow has no initial loader for instances).
onMounted(() => {
    if (!selectedLoader.value) selectedLoader.value = 'vanilla'
})

const loaderVersionTypeItems: LoaderVersionType[] = ['stable', 'latest', 'other']

/** Builds of the selected game version are not recorded in the snapshot. */
const buildsUnresolved = computed(() => {
    const loader = selectedLoader.value
    const gameVersion = selectedGameVersion.value
    if (!loader || !gameVersion || loader === 'vanilla') return false
    const cacheKey = loaderMetadataCacheKey(toApiLoaderName(loader), gameVersion)
    return hasUnresolvedBuilds(ctx.loaderVersionsCache.value[cacheKey], gameVersion)
})

const loaderVersionTypeDisabledItems = computed<LoaderVersionType[]>(() => {
    if (buildsUnresolved.value) return []
    const noStableVersions = !loaderVersionsData.value.some((version) => version.stable)
    return noStableVersions ? ['stable'] : []
})

// Icon (the file picker's `pickInstanceIcon`, i.e. the instance icon picker modal)
const defaultInstanceIconUrl = computed(() => getLoaderInstanceIconUrl(selectedLoader.value))

async function triggerIconInput() {
    const picked = await area.iconPicker.value?.show()
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

// Game directory (`axolotl-minecraft-directories`, read once per mount like the app)
const BUILTIN_GAME_DIR = 'builtin'
const configuredMinecraftDirectories = area.configuredMinecraftDirectories.value
const gameDirOptions = [
    BUILTIN_GAME_DIR,
    ...configuredMinecraftDirectories.map((entry) => entry.path),
]
const configuredMinecraftDirectoryModes = new Map(
    configuredMinecraftDirectories.map((entry) => [entry.path, entry.mode]),
)
if (
    ctx.gameDirOverrideMode.value !== 'builtin' &&
    ctx.gameDirOverride.value &&
    !gameDirOptions.includes(ctx.gameDirOverride.value)
) {
    gameDirOptions.push(ctx.gameDirOverride.value)
}

const gameDirSelection = computed<string>({
    get: () =>
        ctx.gameDirOverrideMode.value === 'builtin'
            ? BUILTIN_GAME_DIR
            : (ctx.gameDirOverride.value ?? BUILTIN_GAME_DIR),
    set: (selection) => {
        if (selection === BUILTIN_GAME_DIR) {
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
const loaderVersionSummary = computed<'loading' | 'selected' | 'empty'>(() => {
    if (loaderVersionsLoading.value) return 'loading'
    return selectedLoaderVersion.value ? 'selected' : 'empty'
})

const loaderVersionSelectorLabels = computed(() => {
    const loading = formatMessage(commonMessages.loadingLabel)
    const empty = formatMessage(messages.noVersionsAvailable)
    const loader = selectedLoader.value
    const gameVersion = selectedGameVersion.value
    let resolvedEmpty = false
    if (loader && gameVersion && loader !== 'vanilla') {
        const cacheKey = loaderMetadataCacheKey(toApiLoaderName(loader), gameVersion)
        resolvedEmpty =
            loaderSupportState(
                ctx.loaderMetadataStatus.value[cacheKey] ?? 'unknown',
                ctx.loaderVersionsCache.value[cacheKey],
                gameVersion,
            ) === 'unsupported'
    }
    if (loaderVersionsLoading.value) return { placeholder: loading, searchPlaceholder: loading }
    if (resolvedEmpty) return { placeholder: empty, searchPlaceholder: empty }
    return {
        placeholder: formatMessage(messages.selectLoaderVersion),
        searchPlaceholder: formatMessage(messages.searchLoaderVersion),
    }
})

function isGameVersionListedByLoader(gameVersion: string, loader: string): boolean {
    if (loader === 'vanilla') return true
    const manifest = ctx.loaderVersionsCache.value[toApiLoaderName(loader)]
    if (!manifest) return false
    return manifest.gameVersions.some(
        (entry) => entry.id === gameVersion && (entry.loaders.length > 0 || entry.unresolvedBuilds),
    )
}

const gameVersionsLoading = computed(() => {
    const loader = selectedLoader.value
    if (!loader || loader === 'vanilla') return false
    const status = ctx.loaderMetadataStatus.value[toApiLoaderName(loader)] ?? 'unknown'
    return status === 'unknown' || status === 'loading'
})

// Game versions from the tag list, filtered by loader support
const gameVersionOptions = computed<(ComboboxOption<string> & { versionType?: string })[]>(() => {
    const versions = area.gameVersions.value
    if (selectedLoader.value && selectedLoader.value !== 'vanilla') {
        const loader = selectedLoader.value
        return versions
            .filter((version) => isGameVersionListedByLoader(version.version, loader))
            .map((version) => ({
                value: version.version,
                label: version.version,
                versionType: version.version_type,
            }))
    }
    return versions.map((version) => ({
        value: version.version,
        label: version.version,
        versionType: version.version_type,
    }))
})

// Filtered game versions based on the selected version type
const filteredGameVersionOptions = computed<ComboboxOption<string>[]>(() =>
    gameVersionOptions.value.filter((option) => {
        if (!option.versionType) return false
        return isVersionTypeMatch(
            option.versionType,
            String(option.value),
            selectedVersionType.value,
        )
    }),
)

const gameVersionMetadataState = computed<GameVersionMetadataState>(() => {
    const loader = selectedLoader.value
    if (!loader || loader === 'vanilla') return 'ready'
    const status = ctx.loaderMetadataStatus.value[toApiLoaderName(loader)] ?? 'unknown'
    if (status === 'unknown' || status === 'loading') return 'loading'
    if (status === 'error') return 'error'
    return filteredGameVersionOptions.value.length > 0 ? 'ready' : 'empty'
})

const gameVersionSelectorLabels = computed(() => {
    const state = gameVersionMetadataState.value
    if (state === 'loading') {
        const loading = formatMessage(commonMessages.loadingLabel)
        return { placeholder: loading, searchPlaceholder: loading }
    }
    if (state === 'empty') {
        const empty = formatMessage(messages.noGameVersionsForLoader)
        return { placeholder: empty, searchPlaceholder: empty }
    }
    if (state === 'error') {
        const error = formatMessage(messages.gameVersionsLoadFailed)
        return { placeholder: error, searchPlaceholder: error }
    }
    return {
        placeholder: formatMessage(messages.selectGameVersion),
        searchPlaceholder: formatMessage(messages.searchGameVersion),
    }
})

const gameVersionNoOptionsMessage = computed(() => {
    if (gameVersionMetadataState.value === 'loading')
        return formatMessage(commonMessages.loadingLabel)
    if (gameVersionMetadataState.value === 'empty')
        return formatMessage(messages.noGameVersionsForLoader)
    if (gameVersionMetadataState.value === 'error')
        return formatMessage(messages.gameVersionsLoadFailed)
    return formatMessage(messages.noVersionsAvailable)
})

// Select an initial game version without replacing an explicit user selection.
watch(
    gameVersionOptions,
    () => {
        const options = filteredGameVersionOptions.value
        selectedGameVersion.value =
            selectedGameVersion.value ?? (options[0] ? String(options[0].value) : null)
    },
    { immediate: true },
)

// Auto-select the latest game version when the version type changes
watch(
    selectedVersionType,
    () => {
        const options = filteredGameVersionOptions.value
        selectedGameVersion.value = options.length > 0 ? String(options[0].value) : null
    },
    { immediate: true },
)

// Fetch version data when the loader changes so game versions can be filtered
watch(
    () => selectedLoader.value,
    async (loader) => {
        await ctx.fetchLoaderMetadata(loader)
    },
    { immediate: true },
)

// Resolve loader versions for the loader + game version (latest request wins)
let loaderVersionRequest = 0
watch(
    [() => selectedLoader.value, () => selectedGameVersion.value],
    async ([loader, gameVersion]) => {
        const request = ++loaderVersionRequest
        loaderVersionsLoading.value = false
        loaderVersionsData.value = []
        selectedLoaderVersion.value = null

        if (!loader || !gameVersion || loader === 'vanilla') return

        loaderVersionsLoading.value = true
        await ctx.fetchLoaderMetadata(loader, gameVersion)
        if (request !== loaderVersionRequest) return
        const cacheKey = loaderMetadataCacheKey(toApiLoaderName(loader), gameVersion)
        loaderVersionsData.value = loaderVersionsForGameVersion(
            ctx.loaderVersionsCache.value[cacheKey],
            gameVersion,
        )
        loaderVersionsLoading.value = false
        autoSelectLoaderVersion()
    },
)

watch(
    () => loaderVersionType.value,
    () => autoSelectLoaderVersion(),
)

function autoSelectLoaderVersion() {
    if (
        loaderVersionType.value === 'stable' &&
        loaderVersionTypeDisabledItems.value.includes('stable')
    ) {
        loaderVersionType.value = 'latest'
    }
    if (loaderVersionType.value === 'stable') {
        const stable = loaderVersionsData.value.find((version) => version.stable)
        selectedLoaderVersion.value = stable?.id ?? loaderVersionsData.value[0]?.id ?? null
    } else if (loaderVersionType.value === 'latest') {
        selectedLoaderVersion.value = loaderVersionsData.value[0]?.id ?? null
    } else if (loaderVersionType.value === 'other' && !selectedLoaderVersion.value) {
        selectedLoaderVersion.value = loaderVersionsData.value[0]?.id ?? null
    }
}

const loaderVersionOptions = computed<ComboboxOption<string>[]>(() =>
    loaderVersionsData.value.map((version) => ({
        value: version.id,
        label: version.stable ? `${version.id} (stable)` : version.id,
    })),
)
</script>

<template>
    <div class="space-y-6">
        <!-- Icon upload -->
        <div class="flex items-center gap-4">
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

        <!-- Name field -->
        <div data-onboarding-id="creation-name" class="flex flex-col gap-2">
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

        <!-- Game directory isolation -->
        <div data-onboarding-id="creation-game-dir" class="flex flex-col gap-2">
            <span class="font-semibold text-[var(--color-text-primary)]">{{
                formatMessage(messages.gameDirLabel)
            }}</span>
            <RadioButtons v-model="gameDirSelection" :items="gameDirOptions" force-selection>
                <template #default="{ item }">
                    {{ item === BUILTIN_GAME_DIR ? formatMessage(messages.gameDirManaged) : item }}
                </template>
            </RadioButtons>
        </div>

        <!-- Game version -->
        <div data-onboarding-id="creation-game-version" class="flex flex-col gap-2">
            <span class="font-semibold text-[var(--color-text-primary)]">{{
                formatMessage(commonMessages.gameVersionLabel)
            }}</span>
            <div class="flex gap-2">
                <Chips
                    v-model="selectedVersionType"
                    :items="versionTypeItems"
                    :format-label="formatVersionTypeLabel"
                />
            </div>
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
            />
        </div>

        <!-- Loader chips -->
        <div
            v-if="!ctx.hideLoaderChips.value"
            data-onboarding-id="creation-loader"
            class="flex flex-col gap-2"
        >
            <span class="font-semibold text-[var(--color-text-primary)]">{{
                formatMessage(messages.loaderLabel)
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
        <template v-if="!ctx.hideLoaderVersion.value">
            <Collapsible :collapsed="!selectedLoader || !selectedGameVersion" overflow-visible>
                <div class="flex flex-col gap-2">
                    <div class="flex items-center gap-2">
                        <span class="font-semibold text-[var(--color-text-primary)]">{{
                            formatMessage(messages.loaderVersionLabel)
                        }}</span>
                        <span
                            v-if="loaderVersionSummary === 'loading'"
                            class="text-sm text-[var(--color-text-tertiary)]"
                        >
                            {{ formatMessage(commonMessages.loadingLabel) }}
                        </span>
                        <span
                            v-else-if="
                                loaderVersionType !== 'other' && loaderVersionSummary === 'selected'
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
                        v-model="loaderVersionType"
                        :items="loaderVersionTypeItems"
                        :disabled-items="loaderVersionTypeDisabledItems"
                        :disabled-tooltip="formatMessage(messages.noSuchVersionsAvailable)"
                        :format-label="formatLoaderVersionTypeLabel"
                    />
                    <div v-if="loaderVersionType === 'other'">
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
                        />
                    </div>
                </div>
            </Collapsible>
        </template>

        <div v-if="adjunctOptions.length" class="flex flex-col gap-2">
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
