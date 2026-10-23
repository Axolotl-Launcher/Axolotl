<script setup lang="ts">
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { provideInstallationSettings } from '@modrinth/ui/src/layouts/shared/installation-settings/providers/installation-settings.ts'
import { injectNotificationManager } from '@modrinth/ui/src/providers/web-notifications.ts'
import { commonMessages } from '@modrinth/ui/src/utils/common-messages.ts'
import { formatLoaderLabel, instanceInstallablePlatforms } from '@modrinth/ui/src/utils/loaders.ts'
import { computed, ref, shallowRef, useTemplateRef, watch } from 'vue'

import { usePreview } from '../../../lib/state'
import type { GameInstance, InstanceLoader } from '../../../lib/types'
import { useLocalJobs } from '../../downloads/local-jobs'
import SymlinkInstanceWarning from '../components/SymlinkInstanceWarning.vue'
import { installationSettingsMessages as messages } from '../messages'
import { injectInstanceSettings } from './context'
import InstallationSettingsLayout from './installation/InstallationSettingsLayout.vue'
import {
    fetchLoaderVersions,
    type GameVersionTag,
    getGameVersions,
    type LoaderMetadataStatus,
    loaderSupportState,
    loaderVersionsForGameVersion,
    loaderVersionsQuery,
} from './installation/metadata'
import {
    applyPackVersion,
    getLinkedModpackInfo,
    type LinkedModpackInfo,
    loadModpackProject,
    packContent,
    runPackInstall,
    toUpdaterVersion,
} from './installation/modpack'
import { useSimulatedInvoke } from './state'

/**
 * Port of `apps/app-frontend/src/components/ui/instance_settings/InstallationSettings.vue`.
 * The metadata queries read the website's snapshot; `install_existing_instance`
 * queues the same reinstall job the Library's Repair action does, and the
 * modpack operations run as pack install stages on the window's clock.
 */
const { handleError } = injectNotificationManager()
const { formatMessage } = useVIntl()
const preview = usePreview()
const localJobs = useLocalJobs()
const simulate = useSimulatedInvoke()

const { instance, offline, isMinecraftServer, onUnlinked, closeModal } = injectInstanceSettings()
const skipNonEssentialWarnings = computed(
    () => preview.settings.feature_flags.skip_non_essential_warnings ?? false,
)

const gameVersions = shallowRef<GameVersionTag[] | null>(null)
getGameVersions().then(
    (versions) => {
        gameVersions.value = versions
    },
    (error) => handleError(error),
)

const editingPlatform = ref<string>(instance.value.loader)
const editingGameVersion = ref(instance.value.game_version)
const scopedLoader = computed(() =>
    editingPlatform.value === 'neoforge' ? 'neo' : editingPlatform.value,
)
const scopedLoaderQueryEnabled = computed(
    () => editingPlatform.value !== 'vanilla' && !!editingGameVersion.value,
)
watch(
    [scopedLoader, editingGameVersion, scopedLoaderQueryEnabled],
    ([loader, gameVersion, enabled]) => {
        if (enabled) fetchLoaderVersions(loader, gameVersion)
    },
    { immediate: true },
)
const scopedLoaderVersionsQuery = computed(() =>
    scopedLoaderQueryEnabled.value
        ? loaderVersionsQuery(scopedLoader.value, editingGameVersion.value)
        : undefined,
)
const scopedLoaderMetadataStatus = computed<LoaderMetadataStatus>(() => {
    if (!scopedLoaderQueryEnabled.value) return 'unknown'
    const query = scopedLoaderVersionsQuery.value
    if (!query || query.status === 'pending') return 'loading'
    if (query.status === 'error') return 'error'
    return 'success'
})
const scopedLoaderVersionState = computed(() =>
    loaderSupportState(
        scopedLoaderMetadataStatus.value,
        scopedLoaderVersionsQuery.value?.data,
        editingGameVersion.value,
    ),
)
const metadataLoading = computed(() => gameVersions.value === null)

const isModrinthLinkedModpack = computed(() => instance.value.link?.type === 'modrinth_modpack')
const isCurseForgeLinkedModpack = computed(() => false)
const isLinkedManagedModpack = computed(
    () => isModrinthLinkedModpack.value || isCurseForgeLinkedModpack.value,
)
const isImportedModpack = computed(() => instance.value.link?.type === 'imported_modpack')

/** `useQuery(['linkedModpackInfo', id])`: loading only until the first answer. */
const modpackInfo = shallowRef<LinkedModpackInfo | null>(null)
const modpackInfoLoading = ref(false)
const modpackInfoEnabled = computed(
    () => instance.value.install_stage === 'installed' && isLinkedManagedModpack.value && !offline,
)
watch(
    () =>
        [
            modpackInfoEnabled.value,
            instance.value.id,
            instance.value.link?.project_id,
            instance.value.link?.type === 'modrinth_modpack'
                ? instance.value.link.version_id
                : null,
            instance.value.update_channel,
        ] as const,
    async ([enabled]) => {
        if (!enabled) return
        if (!modpackInfo.value) modpackInfoLoading.value = true
        try {
            const info = await getLinkedModpackInfo(instance.value)
            modpackInfo.value = info
        } catch (error) {
            handleError(error)
        } finally {
            modpackInfoLoading.value = false
        }
    },
    { immediate: true },
)

const repairing = ref(false)
const reinstalling = ref(false)

function linkVersionId(target: GameInstance): string {
    const link = target.link
    return link && 'version_id' in link ? (link.version_id ?? '') : ''
}

/** `install_existing_instance(id, force)`: the instance's reinstall job. */
async function installExistingInstance(target: GameInstance) {
    await simulate(160)
    localJobs.repair(target)
}

/** `pickModpackFile` + `install_pack_to_existing_instance`: the browser's picker stands in. */
const modpackInput = useTemplateRef<HTMLInputElement>('modpackInput')
let resolveModpackPick: ((file: File | null) => void) | null = null

function pickModpackFile(): Promise<File | null> {
    resolveModpackPick?.(null)
    return new Promise((resolve) => {
        resolveModpackPick = resolve
        const input = modpackInput.value
        if (!input) {
            resolve(null)
            return
        }
        input.value = ''
        input.click()
        // A dismissed picker never fires `change`; `cancel` reports it where supported.
        input.addEventListener(
            'cancel',
            () => {
                if (resolveModpackPick === resolve) {
                    resolveModpackPick = null
                    resolve(null)
                }
            },
            { once: true },
        )
    })
}

function handleModpackInput(event: Event) {
    const file = (event.target as HTMLInputElement).files?.[0] ?? null
    const resolve = resolveModpackPick
    resolveModpackPick = null
    resolve?.(file)
}

async function installLocalModpackFromPicker() {
    const picked = await pickModpackFile()
    if (!picked) return false

    await simulate(200)
    const target = instance.value
    runPackInstall(preview, target.id, (current) => {
        if (current.link?.type === 'imported_modpack') {
            current.link = {
                ...current.link,
                filename: picked.name,
                name: current.link.name ?? picked.name.replace(/\.(mrpack|zip)$/i, ''),
            }
        }
    })
    return true
}

provideInstallationSettings({
    closeSettings: closeModal,
    loading: computed(() => metadataLoading.value || modpackInfoLoading.value),
    installationInfo: computed(() => {
        const rows = [
            {
                label: formatMessage(commonMessages.platformLabel),
                value: formatLoaderLabel(instance.value.loader),
            },
            {
                label: formatMessage(commonMessages.gameVersionLabel),
                value: instance.value.game_version,
            },
        ]
        if ((instance.value.loader_components ?? []).length > 1) {
            for (const component of instance.value.loader_components ?? []) {
                rows.push({
                    label: formatMessage(messages.loaderVersion, {
                        loader: formatLoaderLabel(component.kind),
                    }),
                    value: component.version ?? formatLoaderLabel(component.kind),
                })
            }
        } else if (instance.value.loader !== 'vanilla' && instance.value.loader_version) {
            rows.push({
                label: formatMessage(messages.loaderVersion, {
                    loader: formatLoaderLabel(instance.value.loader),
                }),
                value: instance.value.loader_version,
            })
        }
        return rows
    }),
    isLinked: computed(() => isLinkedManagedModpack.value || isImportedModpack.value),
    isBusy: computed(
        () =>
            instance.value.install_stage !== 'installed' ||
            repairing.value ||
            reinstalling.value ||
            !!offline,
    ),
    skipNonEssentialWarnings,
    modpack: computed(() => {
        const link = instance.value.link
        if (isImportedModpack.value && link?.type === 'imported_modpack') {
            return {
                iconUrl: instance.value.icon_path,
                title: link.name ?? instance.value.name,
                versionNumber: link.version_number ?? undefined,
                filename: link.filename ?? undefined,
            }
        }
        if (modpackInfo.value) {
            return {
                iconUrl: modpackInfo.value.project.icon_url,
                title: modpackInfo.value.project.title,
                // The preview's project page resolves projects by id.
                link: `/project/${modpackInfo.value.project.id}`,
                versionNumber: modpackInfo.value.version?.version_number,
            }
        }
        // Fallback when linked metadata is temporarily unavailable so the
        // association controls still match Modrinth-linked packs.
        if (isModrinthLinkedModpack.value && link?.type === 'modrinth_modpack') {
            return {
                iconUrl: instance.value.icon_path,
                title: instance.value.name,
                link: `/project/${link.project_id}`,
                versionNumber: link.version_id ?? undefined,
            }
        }
        return null
    }),
    currentPlatform: computed(() => instance.value.loader),
    currentGameVersion: computed(() => instance.value.game_version),
    currentLoaderVersion: computed(() => instance.value.loader_version ?? ''),
    availablePlatforms: computed(() => [...instanceInstallablePlatforms]),
    editingPlatformRef: editingPlatform,
    editingGameVersionRef: editingGameVersion,
    loaderVersionState: scopedLoaderVersionState,

    resolveGameVersions(_loader, showSnapshots) {
        const versions = gameVersions.value ?? []
        return (
            showSnapshots ? versions : versions.filter((x) => x.version_type === 'release')
        ).map((x) => ({ value: x.version, label: x.version }))
    },

    resolveLoaderVersions(loader, gameVersion) {
        if (loader === 'vanilla' || !gameVersion) return []
        if (loader !== editingPlatform.value || gameVersion !== editingGameVersion.value) return []
        if (scopedLoaderVersionState.value !== 'supported') return []
        return loaderVersionsForGameVersion(scopedLoaderVersionsQuery.value?.data, gameVersion)
    },

    resolveHasSnapshots() {
        const versions = gameVersions.value ?? []
        return versions.some((x) => x.version_type !== 'release')
    },

    async save(platform, gameVersion, loaderVersionId) {
        try {
            await simulate()
            const target = preview.instances.get(instance.value.id)
            if (!target) throw new Error('Unknown instance')
            const loader = platform as InstanceLoader
            target.loader = loader
            target.game_version = gameVersion
            if (platform !== 'vanilla' && loaderVersionId) {
                target.loader_version = loaderVersionId
            } else if (platform === 'vanilla') {
                target.loader_version = undefined
            }
            target.loader_components =
                platform !== 'vanilla' && target.loader_version
                    ? [
                          {
                              instanceId: target.id,
                              kind: loader,
                              version: target.loader_version,
                              role: 'primary',
                          },
                      ]
                    : []
            target.modified = new Date().toISOString()
        } catch (error) {
            handleError(error)
        }
    },

    afterSave: async () => {
        await installExistingInstance(instance.value).catch(handleError)
    },

    async repair() {
        repairing.value = true
        await installExistingInstance(instance.value).catch(handleError)
        repairing.value = false
    },

    async reinstallModpack() {
        reinstalling.value = true
        try {
            if (isImportedModpack.value) {
                await installLocalModpackFromPicker()
            } else {
                // `update_repair_modrinth`: the pack version is installed again from scratch.
                await simulate(240)
                const link = instance.value.link
                const projectId = link?.type === 'modrinth_modpack' ? link.project_id : null
                const files = projectId ? await packContent(projectId) : null
                runPackInstall(preview, instance.value.id, (current) => {
                    if (!files) return
                    preview.downloads.contents.value = {
                        ...preview.downloads.contents.value,
                        [current.id]: files,
                    }
                })
            }
        } catch (error) {
            handleError(error)
        } finally {
            reinstalling.value = false
        }
    },

    async swapModpack() {
        reinstalling.value = true
        try {
            await installLocalModpackFromPicker()
        } finally {
            reinstalling.value = false
        }
    },

    async unlinkModpack() {
        await simulate()
        const target = preview.instances.get(instance.value.id)
        if (target) target.link = null
        modpackInfo.value = null
        onUnlinked()
    },

    getCachedModpackVersions: () => null,
    async fetchModpackVersions() {
        const link = instance.value.link
        if (link?.type !== 'modrinth_modpack') return []
        try {
            await simulate(260)
            const project = await loadModpackProject(link.project_id)
            return project.versions.map((version) => toUpdaterVersion(version, project.id)) as never
        } catch (error) {
            handleError(error)
            return []
        }
    },

    async getVersionChangelog(versionId: string) {
        const link = instance.value.link
        if (link?.type !== 'modrinth_modpack') return null
        const project = await loadModpackProject(link.project_id).catch(() => null)
        const version = project?.versions.find((entry) => entry.id === versionId)
        return version ? (toUpdaterVersion(version, link.project_id) as never) : null
    },

    async onModpackVersionConfirm(version) {
        try {
            // `update_managed_modrinth_version`: installs the chosen pack version.
            await simulate(240)
            runPackInstall(preview, instance.value.id, (current) =>
                applyPackVersion(current, version as never),
            )
        } catch (error) {
            handleError(error as Error)
        }
    },

    updaterModalProps: computed(() => ({
        isApp: true,
        currentVersionId:
            modpackInfo.value?.update?.provider === 'modrinth'
                ? modpackInfo.value.update.target_version_id
                : linkVersionId(instance.value),
        projectIconUrl: modpackInfo.value?.project?.icon_url,
        projectName: modpackInfo.value?.project?.title ?? 'Modpack',
        currentGameVersion: instance.value.game_version,
        currentLoader: instance.value.loader,
    })),

    isServer: false,
    isApp: true,
    symlinkTarget: computed(() => instance.value.symlink_target),
    showModpackVersionActions: computed(
        () => isLinkedManagedModpack.value && !isMinecraftServer.value,
    ),
    isLocalFile: isImportedModpack,
    repairing,
    reinstalling,
} as never)
</script>

<template>
    <SymlinkInstanceWarning
        v-if="instance?.symlink_target"
        :symlink-target="instance.symlink_target"
    />
    <InstallationSettingsLayout />
    <input
        ref="modpackInput"
        type="file"
        accept=".mrpack,.zip"
        class="hidden"
        @change="handleModpackInput"
    />
</template>
