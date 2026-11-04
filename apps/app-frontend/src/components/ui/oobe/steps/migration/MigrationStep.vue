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
    defineMessages,
    type ImportableLauncher,
    injectInstanceImport,
    injectNotificationManager,
    useVIntl,
} from '@modrinth/ui'
import { open } from '@tauri-apps/plugin-dialog'
import { computed, inject, onMounted, reactive, ref, watch } from 'vue'

import { syncConfiguredDirectLinks } from '@/helpers/direct-link-sync'
import { type ChooseImportMethod, startLauncherImport } from '@/helpers/launcher-import'
import { readMinecraftRoots, writeMinecraftRoots } from '@/helpers/minecraft-roots'

import { useOobeStep } from '../../context'

type AddedFolder = { path: string; state: 'scanning' | 'added' | 'failed'; message?: string }

const LAUNCHER_NAMES: Record<string, string> = {
    ModrinthApp: 'Modrinth App',
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
    detectedTitle: {
        id: 'app.oobe.migration.detected.title',
        defaultMessage: 'Found on this device',
    },
    scanning: { id: 'app.oobe.migration.scanning', defaultMessage: 'Looking for other launchers…' },
    noneFound: {
        id: 'app.oobe.migration.none-found',
        defaultMessage: 'No other launchers were found. You can still add a folder below.',
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
    folderFailed: {
        id: 'app.oobe.migration.folder-failed',
        defaultMessage: 'No game versions were found in this folder.',
    },
    moreOptions: {
        id: 'app.oobe.migration.more-options',
        defaultMessage: 'More import options',
    },
})

const scanning = ref(true)
const launchers = ref<ImportableLauncher[]>([])
const selected = reactive(new Map<string, Set<string>>())
const importing = ref(false)
const startedCount = ref(0)
const folders = ref<AddedFolder[]>([])

const selectedCount = computed(() =>
    [...selected.values()].reduce((total, names) => total + names.size, 0),
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
    return launcher.name === 'Generic'
        ? formatMessage(messages.genericLauncher)
        : (LAUNCHER_NAMES[launcher.name] ?? launcher.name)
}

function selectionOf(launcher: ImportableLauncher) {
    return selected.get(launcher.name) ?? new Set<string>()
}

function setInstance(launcher: ImportableLauncher, name: string, value: boolean) {
    const names = new Set(selectionOf(launcher))
    if (value) names.add(name)
    else names.delete(name)
    selected.set(launcher.name, names)
}

function setLauncher(launcher: ImportableLauncher, value: boolean) {
    selected.set(
        launcher.name,
        new Set(value ? launcher.instances.map((instance) => instance.name) : []),
    )
}

async function importSelected() {
    if (!chooseImportMethod || selectedCount.value === 0 || importing.value) return
    const entries = launchers.value.flatMap((launcher) =>
        launcher.instances
            .filter((instance) => selectionOf(launcher).has(instance.name))
            .map((instance) => ({
                launcherType: launcher.launcherType ?? launcher.name,
                path: launcher.path,
                instanceName: instance.name,
                instancePath: instance.path,
            })),
    )
    importing.value = true
    try {
        const { confirmed } = await startLauncherImport(entries, chooseImportMethod, handleError)
        if (confirmed > 0) {
            startedCount.value += confirmed
            selected.clear()
        }
    } catch (error) {
        handleError(error as Error)
    } finally {
        importing.value = false
    }
}

async function addFolder() {
    const choice = await open({
        multiple: false,
        directory: true,
        title: formatMessage(messages.selectFolder),
    })
    if (typeof choice !== 'string') return
    const path = choice.trim().replace(/[\\/]+$/, '')
    if (!path || folders.value.some((folder) => folder.path === path)) return

    const folder = reactive<AddedFolder>({ path, state: 'scanning' })
    folders.value = [...folders.value, folder]
    step.setBusy(true)
    try {
        const roots = readMinecraftRoots()
        if (!roots.some((root) => root.path === path)) roots.push({ path, mode: 'automatic' })
        const report = await syncConfiguredDirectLinks(roots)
        if (report.errors.length || report.missing) {
            folder.state = 'failed'
            folder.message = report.errors.join('\n') || formatMessage(messages.folderFailed)
            return
        }
        writeMinecraftRoots(roots)
        folder.state = 'added'
        folder.message = formatMessage(messages.folderAdded, {
            count: report.imported + report.updated,
        })
    } catch (error) {
        folder.state = 'failed'
        folder.message = error instanceof Error ? error.message : String(error)
    } finally {
        step.setBusy(false)
    }
}
</script>

<template>
    <div class="flex flex-col gap-6">
        <section class="flex flex-col gap-3">
            <h2 class="m-0 text-base font-semibold text-[var(--color-text-primary)]">
                {{ formatMessage(messages.detectedTitle) }}
            </h2>

            <div
                v-if="scanning && launchers.length === 0"
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
                v-else-if="launchers.length === 0"
                class="m-0 rounded-[var(--radius-lg)] bg-surface-3 px-4 py-3 text-sm text-[var(--color-text-tertiary)]"
            >
                {{ formatMessage(messages.noneFound) }}
            </p>

            <template v-else>
                <div
                    v-for="launcher in launchers"
                    :key="launcher.name"
                    class="flex flex-col rounded-[var(--radius-lg)] border border-solid border-surface-4 bg-surface-3"
                >
                    <div class="flex items-center gap-3 px-4 py-3">
                        <Checkbox
                            :model-value="selectionOf(launcher).size === launcher.instances.length"
                            :indeterminate="
                                selectionOf(launcher).size > 0 &&
                                selectionOf(launcher).size < launcher.instances.length
                            "
                            :label="launcherName(launcher)"
                            @update:model-value="(value: boolean) => setLauncher(launcher, value)"
                        />
                        <span
                            class="min-w-0 truncate text-xs text-[var(--color-text-tertiary)]"
                            :title="launcher.path"
                            >{{ launcher.path }}</span
                        >
                        <span class="ml-auto shrink-0 text-sm text-[var(--color-text-tertiary)]">
                            {{
                                formatMessage(messages.instanceCount, {
                                    count: launcher.instances.length,
                                })
                            }}
                        </span>
                    </div>
                    <div
                        class="flex flex-wrap gap-x-5 gap-y-2 border-0 border-t border-solid border-surface-4 px-4 py-3"
                    >
                        <Checkbox
                            v-for="instance in launcher.instances"
                            :key="instance.path || instance.name"
                            :model-value="selectionOf(launcher).has(instance.name)"
                            :label="instance.name"
                            @update:model-value="
                                (value: boolean) => setInstance(launcher, instance.name, value)
                            "
                        />
                    </div>
                </div>
                <div class="flex flex-wrap items-center gap-3">
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

        <section class="flex flex-col gap-3">
            <div class="flex flex-col gap-1">
                <h2 class="m-0 text-base font-semibold text-[var(--color-text-primary)]">
                    {{ formatMessage(messages.foldersTitle) }}
                </h2>
                <p class="m-0 text-sm text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.foldersDescription) }}
                </p>
            </div>
            <div v-if="folders.length > 0" class="flex flex-col gap-2">
                <div
                    v-for="folder in folders"
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
                <Button @click="addFolder">
                    <PlusIcon />
                    {{ formatMessage(messages.addFolder) }}
                </Button>
                <Button
                    v-if="showCreationModal"
                    type="quiet"
                    @click="showCreationModal({ skipSetupType: true, initialMode: 'import' })"
                >
                    <FolderSearchIcon />
                    {{ formatMessage(messages.moreOptions) }}
                </Button>
            </div>
        </section>
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
