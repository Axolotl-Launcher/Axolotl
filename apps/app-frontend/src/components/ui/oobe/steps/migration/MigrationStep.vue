<script setup lang="ts">
import {
    CheckIcon,
    FolderOpenIcon,
    FolderSearchIcon,
    ImportIcon,
    PlusIcon,
    SpinnerIcon,
} from '@modrinth/assets'
import {
    Button,
    Checkbox,
    Chips,
    defineMessages,
    type ImportableLauncher,
    injectInstanceImport,
    injectNotificationManager,
    useVIntl,
} from '@modrinth/ui'
import { open } from '@tauri-apps/plugin-dialog'
import { computed, inject, onMounted, reactive, ref, watch } from 'vue'

import { errorText } from '@/helpers/error-text'
import type { DirectLinkSyncReport } from '@/helpers/instance'
import { type ChooseImportMethod, startLauncherImport } from '@/helpers/launcher-import'
import { addMinecraftRoot } from '@/helpers/minecraft-roots'

import { useOobeStep } from '../../context'
import {
    folderKey,
    folderName,
    type FoundGroup,
    type FoundInstance,
    groupFound,
    instancePathOf,
} from './found'

type AddedFolder = {
    path: string
    /** Picked with the folder dialog, or one of the folders found on this device. */
    origin: 'picked' | 'found'
    state: 'scanning' | 'added' | 'failed'
    message?: string
}
type ImportMethod = 'direct' | 'import'

const IMPORT_METHODS: ImportMethod[] = ['direct', 'import']

const LAUNCHER_NAMES: Record<string, string> = {
    ModrinthApp: 'Modrinth',
    MultiMC: 'MultiMC',
    PCL2: 'PCL2',
    PCL2CE: 'PCL2 CE',
    HMCL: 'HMCL',
    GDLauncher: 'GDLauncher',
    ATLauncher: 'ATLauncher',
    Curseforge: 'CurseForge',
    PrismLauncher: 'Prism Launcher',
}

const { formatMessage } = useVIntl()
const { handleError } = injectNotificationManager()
const instanceImport = injectInstanceImport()
const chooseImportMethod = inject<ChooseImportMethod>('chooseImportMethod')
const showCreationModal = inject<
    (options?: { skipSetupType?: boolean; initialMode?: 'custom' | 'import' }) => void
>('showCreationModalWithOptions')
const step = useOobeStep()

const messages = defineMessages({
    officialLauncher: {
        id: 'app.oobe.migration.official-launcher',
        defaultMessage: 'Official launcher',
    },
    detectedTitle: {
        id: 'app.oobe.migration.detected.title',
        defaultMessage: 'Found on this device',
    },
    scanning: { id: 'app.oobe.migration.scanning', defaultMessage: 'Looking for other launchers…' },
    noneFound: {
        id: 'app.oobe.migration.none-found',
        defaultMessage: 'No other launchers were found on this device.',
    },
    genericLauncher: { id: 'app.oobe.migration.generic', defaultMessage: 'Minecraft folder' },
    instanceCount: {
        id: 'app.oobe.migration.instance-count',
        defaultMessage: '{count, plural, one {# instance} other {# instances}}',
    },
    importSelected: {
        id: 'app.oobe.migration.import-selected',
        defaultMessage:
            '{count, plural, =0 {Import} one {Import # instance} other {Import # instances}}',
    },
    importStarted: {
        id: 'app.oobe.migration.import-started',
        defaultMessage:
            '{count, plural, one {# instance is} other {# instances are}} being imported. You can keep going while that runs.',
    },
    method: { id: 'app.oobe.migration.method', defaultMessage: 'How to bring it over' },
    methodDirect: { id: 'app.oobe.migration.method.direct', defaultMessage: 'Use in place' },
    methodImport: { id: 'app.oobe.migration.method.import', defaultMessage: 'Import' },
    directHint: {
        id: 'app.oobe.migration.direct-hint',
        defaultMessage:
            'The launcher manages its {count, plural, one {# version} other {# versions}} directly, without copying anything.',
    },
    useFolder: { id: 'app.oobe.migration.use-folder', defaultMessage: 'Use this folder' },
    foldersTitle: {
        id: 'app.oobe.migration.folders.title',
        defaultMessage: 'Use a .minecraft folder in place',
    },
    foldersDescription: {
        id: 'app.oobe.migration.folders.description',
        defaultMessage:
            'The launcher manages the versions in it directly, without copying anything.',
    },
    addFolder: { id: 'app.oobe.migration.add-folder', defaultMessage: 'Add .minecraft folder' },
    selectFolder: {
        id: 'app.oobe.migration.select-folder',
        defaultMessage: 'Select a .minecraft folder',
    },
    folderAdded: {
        id: 'app.oobe.migration.folder-added',
        defaultMessage:
            '{count, plural, =0 {Added} one {Added, # version found} other {Added, # versions found}}',
    },
    moreOptions: {
        id: 'app.oobe.migration.more-options',
        defaultMessage: 'More import options',
    },
})

const scanning = ref(true)
const launchers = ref<ImportableLauncher[]>([])
const groups = computed(() => groupFound(launchers.value))
/** How each found `.minecraft` folder comes over; using it in place unless the user says otherwise. */
const methods = reactive(new Map<string, ImportMethod>())
/** The instances ticked for import, by group key and instance path. */
const selected = reactive(new Map<string, Set<string>>())
const importing = ref(false)
const startedCount = ref(0)
const folders = ref<AddedFolder[]>([])
const pickedFolders = computed(() => folders.value.filter((folder) => folder.origin === 'picked'))

const importable = computed(() => groups.value.some((group) => methodOf(group) === 'import'))
const selectedCount = computed(() =>
    groups.value
        .filter((group) => methodOf(group) === 'import')
        .reduce((total, group) => total + selectionOf(group).size, 0),
)

watch(
    [startedCount, () => folders.value.some((folder) => folder.state === 'added')],
    ([started, addedFolder]) => step.setReady(started > 0 || addedFolder),
    { immediate: true },
)

onMounted(async () => {
    try {
        launchers.value = await instanceImport.getDetectedLaunchers((launcher) => {
            if (!launchers.value.some((entry) => entry.name === launcher.name)) {
                launchers.value = [...launchers.value, launcher]
            }
        })
    } catch (error) {
        console.warn('Failed to look for other launchers', error)
    } finally {
        scanning.value = false
    }
})

function launcherName(launcher: ImportableLauncher) {
    if (launcher.name === 'MinecraftLauncher') return formatMessage(messages.officialLauncher)
    return launcher.name === 'Generic'
        ? formatMessage(messages.genericLauncher)
        : (LAUNCHER_NAMES[launcher.name] ?? launcher.name)
}

function methodOf(group: FoundGroup): ImportMethod {
    return group.root ? (methods.get(group.key) ?? 'direct') : 'import'
}

function methodLabel(method: ImportMethod) {
    return formatMessage(method === 'direct' ? messages.methodDirect : messages.methodImport)
}

function folderAt(path: string) {
    return folders.value.find((folder) => folderKey(folder.path) === folderKey(path))
}

/** A folder in use (or being set up) can no longer switch to being copied. */
function methodLocked(group: FoundGroup) {
    const folder = group.root ? folderAt(group.root) : undefined
    return !!folder && folder.state !== 'failed'
}

function instanceId(instance: FoundInstance) {
    return folderKey(instancePathOf(instance))
}

/** A version's own folder name, without the launcher's folder prefix. */
function instanceLabel(group: FoundGroup, instance: FoundInstance) {
    return group.root ? folderName(instancePathOf(instance)) : instance.name
}

function selectionOf(group: FoundGroup) {
    return selected.get(group.key) ?? new Set<string>()
}

function setInstance(group: FoundGroup, instance: FoundInstance, value: boolean) {
    const ids = new Set(selectionOf(group))
    if (value) ids.add(instanceId(instance))
    else ids.delete(instanceId(instance))
    selected.set(group.key, ids)
}

function setGroup(group: FoundGroup, value: boolean) {
    selected.set(group.key, new Set(value ? group.instances.map(instanceId) : []))
}

async function importSelected() {
    if (!chooseImportMethod || selectedCount.value === 0 || importing.value) return
    const entries = groups.value
        .filter((group) => methodOf(group) === 'import')
        .flatMap((group) =>
            group.instances
                .filter((instance) => selectionOf(group).has(instanceId(instance)))
                .map((instance) => ({
                    launcherType: group.launcher.launcherType ?? group.launcher.name,
                    path: group.launcher.path,
                    instanceName: instance.name,
                    instancePath: instancePathOf(instance),
                })),
        )
    importing.value = true
    try {
        const { confirmed } = await startLauncherImport(entries, chooseImportMethod, handleError)
        if (confirmed > 0) {
            step.markChanged()
            startedCount.value += confirmed
            selected.clear()
        }
    } catch (error) {
        handleError(error as Error)
    } finally {
        importing.value = false
    }
}

/**
 * Whether a sync error is about `root`. The report covers every configured root and each
 * error names the path it concerns, so one stale folder must not fail a new, valid one.
 */
function concerns(message: string, root: string) {
    const key = (value: string) => value.replaceAll('\\', '/').toLowerCase()
    const text = key(message)
    const path = key(root)
    for (let at = text.indexOf(path); at !== -1; at = text.indexOf(path, at + 1)) {
        const next = text.charAt(at + path.length)
        if (next === '' || next === '/' || next === ':' || next === ' ') return true
    }
    return false
}

async function pickFolder() {
    const choice = await open({
        multiple: false,
        directory: true,
        title: formatMessage(messages.selectFolder),
    })
    if (typeof choice === 'string') await linkFolder(choice, 'picked')
}

/** Adds `.minecraft` folder `choice` to the folders the launcher manages in place. */
async function linkFolder(choice: string, origin: AddedFolder['origin']) {
    const path = choice.trim().replace(/[\\/]+$/, '')
    if (!path) return
    const existing = folderAt(path)
    if (existing && existing.state !== 'failed') return

    const folder = reactive<AddedFolder>({ path, origin, state: 'scanning' })
    folders.value = [...folders.value.filter((entry) => entry !== existing), folder]
    step.setBusy(true)
    try {
        const ownErrors = (report: DirectLinkSyncReport) =>
            report.errors.filter((message) => concerns(message, path))
        const { report, added, kept } = await addMinecraftRoot(
            path,
            (result) => ownErrors(result).length === 0,
        )
        if (!kept) {
            folder.state = 'failed'
            folder.message = ownErrors(report).join('\n')
            return
        }
        if (added || report.imported + report.updated + report.removed > 0) step.markChanged()
        folder.state = 'added'
        folder.message = formatMessage(messages.folderAdded, {
            count: report.imported + report.updated,
        })
    } catch (error) {
        folder.state = 'failed'
        folder.message = errorText(error)
    } finally {
        step.setBusy(false)
    }
}
</script>

<template>
    <div class="flex flex-col gap-6">
        <section class="flex flex-col gap-3">
            <div class="flex flex-col gap-1">
                <h2 class="m-0 text-base font-semibold text-[var(--color-text-primary)]">
                    {{ formatMessage(messages.foldersTitle) }}
                </h2>
                <p class="m-0 text-sm text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.foldersDescription) }}
                </p>
            </div>
            <div v-if="pickedFolders.length > 0" class="flex flex-col gap-2">
                <div
                    v-for="folder in pickedFolders"
                    :key="folder.path"
                    class="flex items-center gap-3 rounded-[var(--radius-lg)] bg-surface-3 px-4 py-3"
                >
                    <FolderOpenIcon class="size-5 shrink-0 text-[var(--color-text-tertiary)]" />
                    <span class="flex min-w-0 flex-1 flex-col">
                        <span class="truncate text-sm font-medium text-[var(--color-text-primary)]">
                            {{ folder.path }}
                        </span>
                        <span
                            v-if="folder.message"
                            class="text-xs"
                            :class="
                                folder.state === 'failed'
                                    ? 'text-[var(--color-red)]'
                                    : 'text-[var(--color-text-tertiary)]'
                            "
                        >
                            {{ folder.message }}
                        </span>
                    </span>
                    <SpinnerIcon
                        v-if="folder.state === 'scanning'"
                        class="size-4 shrink-0 animate-spin"
                    />
                    <CheckIcon
                        v-else-if="folder.state === 'added'"
                        class="size-5 shrink-0 text-brand"
                    />
                </div>
            </div>
            <div class="flex flex-wrap gap-2">
                <Button @click="pickFolder">
                    <PlusIcon />
                    {{ formatMessage(messages.addFolder) }}
                </Button>
            </div>
        </section>

        <section class="flex flex-col gap-3">
            <h2 class="m-0 text-base font-semibold text-[var(--color-text-primary)]">
                {{ formatMessage(messages.detectedTitle) }}
            </h2>

            <div
                v-if="scanning && groups.length === 0"
                class="flex flex-col gap-2"
                aria-busy="true"
            >
                <div
                    v-for="index in 2"
                    :key="index"
                    class="flex items-center gap-3 rounded-[var(--radius-lg)] bg-surface-3 px-4 py-3"
                >
                    <span class="oobe-skeleton size-9 rounded-[var(--radius-sm)]" />
                    <span class="flex flex-1 flex-col gap-2">
                        <span class="oobe-skeleton h-3 w-32 rounded-full" />
                        <span class="oobe-skeleton h-2.5 w-52 rounded-full" />
                    </span>
                </div>
                <p class="m-0 flex items-center gap-2 text-sm text-[var(--color-text-tertiary)]">
                    <SpinnerIcon class="size-4 animate-spin" />
                    {{ formatMessage(messages.scanning) }}
                </p>
            </div>

            <p
                v-else-if="groups.length === 0"
                class="m-0 rounded-[var(--radius-lg)] bg-surface-3 px-4 py-3 text-sm text-[var(--color-text-tertiary)]"
            >
                {{ formatMessage(messages.noneFound) }}
            </p>

            <template v-else>
                <div
                    v-for="group in groups"
                    :key="group.key"
                    class="flex flex-col rounded-[var(--radius-lg)] border border-solid border-surface-4 bg-surface-3"
                >
                    <div v-if="group.root" class="flex items-center gap-3 px-4 py-3">
                        <FolderOpenIcon class="size-5 shrink-0 text-[var(--color-text-tertiary)]" />
                        <span class="flex min-w-0 flex-1 flex-col">
                            <span
                                class="truncate text-sm font-medium text-[var(--color-text-primary)]"
                            >
                                {{
                                    group.launcher.name === 'MinecraftLauncher'
                                        ? launcherName(group.launcher)
                                        : folderName(group.root)
                                }}
                            </span>
                            <span
                                class="truncate text-xs text-[var(--color-text-tertiary)]"
                                :title="group.root"
                                >{{ group.root }}</span
                            >
                        </span>
                        <Chips
                            class="shrink-0"
                            :model-value="methodOf(group)"
                            :items="IMPORT_METHODS"
                            :format-label="methodLabel"
                            :aria-label="formatMessage(messages.method)"
                            :disabled-items="methodLocked(group) ? IMPORT_METHODS : []"
                            :capitalize="false"
                            size="small"
                            hide-checkmark-icon
                            @update:model-value="
                                (method: ImportMethod | null | undefined) =>
                                    method && methods.set(group.key, method)
                            "
                        />
                    </div>
                    <div v-else class="flex items-center gap-3 px-4 py-3">
                        <Checkbox
                            class="shrink-0"
                            :model-value="selectionOf(group).size === group.instances.length"
                            :indeterminate="
                                selectionOf(group).size > 0 &&
                                selectionOf(group).size < group.instances.length
                            "
                            :label="launcherName(group.launcher)"
                            @update:model-value="(value: boolean) => setGroup(group, value)"
                        />
                        <span
                            class="min-w-0 truncate text-xs text-[var(--color-text-tertiary)]"
                            :title="group.launcher.path"
                            >{{ group.launcher.path }}</span
                        >
                        <span class="ml-auto shrink-0 text-sm text-[var(--color-text-tertiary)]">
                            {{
                                formatMessage(messages.instanceCount, {
                                    count: group.instances.length,
                                })
                            }}
                        </span>
                    </div>

                    <div
                        v-if="group.root && methodOf(group) === 'direct'"
                        class="flex items-center gap-3 border-0 border-t border-solid border-surface-4 px-4 py-3"
                    >
                        <span
                            class="min-w-0 flex-1 text-sm"
                            :class="
                                folderAt(group.root)?.state === 'failed'
                                    ? 'text-[var(--color-red)]'
                                    : 'text-[var(--color-text-tertiary)]'
                            "
                        >
                            {{
                                folderAt(group.root)?.message ??
                                formatMessage(messages.directHint, {
                                    count: group.instances.length,
                                })
                            }}
                        </span>
                        <SpinnerIcon
                            v-if="folderAt(group.root)?.state === 'scanning'"
                            class="size-4 shrink-0 animate-spin"
                        />
                        <CheckIcon
                            v-else-if="folderAt(group.root)?.state === 'added'"
                            class="size-5 shrink-0 text-brand"
                        />
                        <Button v-else class="shrink-0" @click="linkFolder(group.root, 'found')">
                            <FolderOpenIcon />
                            {{ formatMessage(messages.useFolder) }}
                        </Button>
                    </div>
                    <div
                        v-else
                        class="flex flex-wrap gap-x-5 gap-y-2 border-0 border-t border-solid border-surface-4 px-4 py-3"
                    >
                        <Checkbox
                            v-for="instance in group.instances"
                            :key="instanceId(instance)"
                            :model-value="selectionOf(group).has(instanceId(instance))"
                            :label="instanceLabel(group, instance)"
                            @update:model-value="
                                (value: boolean) => setInstance(group, instance, value)
                            "
                        />
                    </div>
                </div>
                <div v-if="importable" class="flex flex-wrap items-center gap-3">
                    <Button
                        type="colored"
                        color="brand"
                        :disabled="selectedCount === 0 || importing || !chooseImportMethod"
                        @click="importSelected"
                    >
                        <SpinnerIcon v-if="importing" class="animate-spin" />
                        <ImportIcon v-else />
                        {{ formatMessage(messages.importSelected, { count: selectedCount }) }}
                    </Button>
                    <span
                        v-if="startedCount > 0"
                        class="flex items-center gap-1.5 text-sm text-[var(--color-text-tertiary)]"
                        role="status"
                    >
                        <CheckIcon class="size-4 text-brand" />
                        {{ formatMessage(messages.importStarted, { count: startedCount }) }}
                    </span>
                </div>
            </template>
        </section>

        <div v-if="showCreationModal">
            <Button
                type="quiet"
                @click="showCreationModal({ skipSetupType: true, initialMode: 'import' })"
            >
                <FolderSearchIcon />
                {{ formatMessage(messages.moreOptions) }}
            </Button>
        </div>
    </div>
</template>

<style scoped lang="scss">
.oobe-skeleton {
    background: linear-gradient(
        90deg,
        var(--surface-4) 0%,
        var(--surface-5) 50%,
        var(--surface-4) 100%
    );
    background-size: 200% 100%;
    animation: oobe-skeleton 1.4s ease-in-out infinite;
}

@keyframes oobe-skeleton {
    from {
        background-position: 100% 0;
    }
    to {
        background-position: -100% 0;
    }
}

@media (prefers-reduced-motion: reduce) {
    .oobe-skeleton {
        animation: none;
    }
}
</style>
