<script setup lang="ts">
import {
    ChevronDownIcon,
    ChevronLeftIcon,
    ChevronRightIcon,
    FileArchiveIcon,
    FolderIcon,
    SearchIcon,
    SpinnerIcon,
} from '@modrinth/assets'
import {
    Button,
    defineMessages,
    NewModal,
    StyledInput,
    useFormatBytes,
    useVIntl,
    useVirtualScroll,
} from '@modrinth/ui'
import { computed, nextTick, ref, useTemplateRef } from 'vue'

import InstanceIcon from '@/components/ui/InstanceIcon.vue'
import { list } from '@/helpers/instance'
import type { GameInstance } from '@/helpers/types.d.ts'
import {
    type InstanceSchematicFile,
    listInstanceSchematics,
    type SchematicPreviewSource,
} from '@/lab/schematic-preview/backend'
import {
    buildInstanceSchematicRows,
    collectSchematicFolders,
    type InstanceSchematicRow,
} from '@/lab/schematic-preview/instance-files'

const emit = defineEmits<{
    open: [source: SchematicPreviewSource, instance: GameInstance]
}>()

const { formatMessage, locale } = useVIntl()
const formatBytes = useFormatBytes()
const modal = useTemplateRef<InstanceType<typeof NewModal>>('modal')
const searchInput = useTemplateRef<InstanceType<typeof StyledInput>>('searchInput')
const instances = ref<GameInstance[]>([])
const selectedInstanceId = ref('')
const files = ref<InstanceSchematicFile[]>([])
const search = ref('')
const expandedFolders = ref<Set<string>>(new Set())
const loadingInstances = ref(false)
const loadingFiles = ref(false)
const error = ref('')
let fileRequest = 0

const messages = defineMessages({
    title: {
        id: 'app.lab.schematic-preview.instance-picker.title',
        defaultMessage: 'Open from an instance',
    },
    chooseInstance: {
        id: 'app.lab.schematic-preview.instance-picker.choose-instance',
        defaultMessage: 'Choose the instance that contains the schematic',
    },
    searchInstances: {
        id: 'app.lab.schematic-preview.instance-picker.search-instances',
        defaultMessage: 'Search instances',
    },
    searchSchematics: {
        id: 'app.lab.schematic-preview.instance-picker.search',
        defaultMessage: 'Search schematics',
    },
    noInstances: {
        id: 'app.lab.schematic-preview.instance-picker.no-instances',
        defaultMessage: 'No installed instances are available.',
    },
    noMatchingInstances: {
        id: 'app.lab.schematic-preview.instance-picker.no-matching-instances',
        defaultMessage: 'No instances match your search.',
    },
    noSchematics: {
        id: 'app.lab.schematic-preview.instance-picker.empty',
        defaultMessage: 'No .litematic or .schem files were found in this instance.',
    },
    noMatchingSchematics: {
        id: 'app.lab.schematic-preview.instance-picker.no-matching-schematics',
        defaultMessage: 'No schematics match your search.',
    },
    back: {
        id: 'app.lab.schematic-preview.instance-picker.back',
        defaultMessage: 'Back to instances',
    },
    selectInstance: {
        id: 'app.lab.schematic-preview.instance-picker.select-instance',
        defaultMessage: 'Browse schematics in {name}',
    },
    openSchematic: {
        id: 'app.lab.schematic-preview.instance-picker.open',
        defaultMessage: 'Open {name}',
    },
    expandFolder: {
        id: 'app.lab.schematic-preview.instance-picker.expand-folder',
        defaultMessage: 'Expand folder {name}',
    },
    collapseFolder: {
        id: 'app.lab.schematic-preview.instance-picker.collapse-folder',
        defaultMessage: 'Collapse folder {name}',
    },
})

const selectedInstance = computed(() =>
    instances.value.find((instance) => instance.id === selectedInstanceId.value),
)
const visibleInstances = computed(() => {
    const query = search.value.trim().toLocaleLowerCase(locale.value)
    return instances.value.filter((instance) => {
        if (!query) return true
        return [instance.name, instance.loader, instance.game_version].some((value) =>
            value.toLocaleLowerCase(locale.value).includes(query),
        )
    })
})
const visibleRows = computed<InstanceSchematicRow[]>(() =>
    buildInstanceSchematicRows(files.value, expandedFolders.value, search.value, locale.value),
)
const { listContainer, totalHeight, visibleTop, visibleItems } = useVirtualScroll(visibleRows, {
    itemHeight: 64,
    bufferSize: 6,
})

function formatModified(value?: number) {
    if (!value) return ''
    return new Intl.DateTimeFormat(locale.value, {
        dateStyle: 'medium',
        timeStyle: 'short',
    }).format(new Date(value * 1000))
}

function toggleFolder(path: string) {
    const next = new Set(expandedFolders.value)
    if (next.has(path)) {
        next.delete(path)
    } else {
        next.add(path)
    }
    expandedFolders.value = next
}

function rowPadding(depth: number) {
    return { paddingLeft: `${0.75 + depth * 1.25}rem` }
}

function rowKey(row: InstanceSchematicRow) {
    return row.kind === 'folder' ? row.path : row.file.relativePath
}

async function loadFiles(instanceId = selectedInstanceId.value) {
    const request = ++fileRequest
    if (!instanceId) {
        files.value = []
        return
    }

    loadingFiles.value = true
    error.value = ''
    try {
        const result = await listInstanceSchematics(instanceId)
        if (request !== fileRequest || instanceId !== selectedInstanceId.value) return
        files.value = result
        expandedFolders.value = new Set(collectSchematicFolders(result))
    } catch (caught) {
        if (request !== fileRequest || instanceId !== selectedInstanceId.value) return
        files.value = []
        error.value = caught instanceof Error ? caught.message : String(caught)
    } finally {
        if (request === fileRequest) loadingFiles.value = false
    }
}

async function show(preferredInstanceId?: string) {
    fileRequest += 1
    selectedInstanceId.value = ''
    files.value = []
    search.value = ''
    expandedFolders.value = new Set()
    error.value = ''
    loadingFiles.value = false
    loadingInstances.value = true
    modal.value?.show()

    try {
        instances.value = (await list())
            .filter((instance) => instance.install_stage === 'installed')
            .sort((left, right) => {
                const lastPlayed =
                    Number(new Date(right.last_played ?? 0)) -
                    Number(new Date(left.last_played ?? 0))
                return lastPlayed || left.name.localeCompare(right.name, locale.value)
            })

        const preferredInstance = instances.value.find(
            (instance) => instance.id === preferredInstanceId,
        )
        if (preferredInstance) {
            selectedInstanceId.value = preferredInstance.id
            await loadFiles(preferredInstance.id)
        }
    } catch (caught) {
        instances.value = []
        files.value = []
        error.value = caught instanceof Error ? caught.message : String(caught)
    } finally {
        loadingInstances.value = false
        await nextTick()
        searchInput.value?.focus()
    }
}

async function selectInstance(instance: GameInstance) {
    selectedInstanceId.value = instance.id
    files.value = []
    search.value = ''
    expandedFolders.value = new Set()
    await loadFiles(instance.id)
    await nextTick()
    searchInput.value?.focus()
}

async function backToInstances() {
    fileRequest += 1
    selectedInstanceId.value = ''
    files.value = []
    search.value = ''
    expandedFolders.value = new Set()
    error.value = ''
    loadingFiles.value = false
    await nextTick()
    searchInput.value?.focus()
}

function openFile(file: InstanceSchematicFile) {
    const instance = selectedInstance.value
    if (!instance) return
    emit(
        'open',
        { kind: 'instance', instanceId: instance.id, relativePath: file.relativePath },
        instance,
    )
    modal.value?.hide()
}

defineExpose({ show })
</script>

<template>
    <NewModal
        ref="modal"
        :header="formatMessage(messages.title)"
        width="min(640px, calc(100vw - 2rem))"
        max-width="640px"
        scrollable
        max-content-height="min(40rem, 78vh)"
    >
        <div class="flex min-h-[24rem] min-w-0 flex-col gap-4">
            <template v-if="!selectedInstance">
                <p class="m-0 text-sm text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.chooseInstance) }}
                </p>
                <StyledInput
                    ref="searchInput"
                    v-model="search"
                    :icon="SearchIcon"
                    type="search"
                    :placeholder="formatMessage(messages.searchInstances)"
                    wrapper-class="w-full"
                    clearable
                />

                <div
                    v-if="loadingInstances"
                    class="flex flex-1 items-center justify-center text-[var(--color-text-tertiary)]"
                >
                    <SpinnerIcon class="size-6 animate-spin" />
                </div>
                <p
                    v-else-if="error"
                    class="m-0 flex flex-1 items-center justify-center text-center text-brand-red"
                >
                    {{ error }}
                </p>
                <p
                    v-else-if="instances.length === 0"
                    class="m-0 flex flex-1 items-center justify-center text-center text-[var(--color-text-tertiary)]"
                >
                    {{ formatMessage(messages.noInstances) }}
                </p>
                <p
                    v-else-if="visibleInstances.length === 0"
                    class="m-0 flex flex-1 items-center justify-center text-center text-[var(--color-text-tertiary)]"
                >
                    {{ formatMessage(messages.noMatchingInstances) }}
                </p>
                <ul v-else class="m-0 flex list-none flex-col gap-1 p-0">
                    <li v-for="instance in visibleInstances" :key="instance.id" class="min-w-0">
                        <button
                            type="button"
                            class="flex min-h-16 w-full cursor-pointer items-center gap-3 rounded-lg border-0 bg-transparent px-3 py-2 text-left text-[var(--color-text-default)] transition-colors hover:bg-surface-4 focus-visible:outline-none focus-visible:ring-4 focus-visible:ring-brand-shadow"
                            :aria-label="
                                formatMessage(messages.selectInstance, { name: instance.name })
                            "
                            @click="selectInstance(instance)"
                        >
                            <InstanceIcon
                                class="size-10 shrink-0"
                                :icon-path="instance.icon_path"
                                :instance-id="instance.id"
                                :loader="instance.loader"
                            />
                            <span class="flex min-w-0 flex-1 flex-col gap-0.5">
                                <strong class="truncate text-[var(--color-text-primary)]">{{
                                    instance.name
                                }}</strong>
                                <span
                                    class="truncate text-sm capitalize text-[var(--color-text-tertiary)]"
                                >
                                    {{ instance.loader }} {{ instance.game_version }}
                                </span>
                            </span>
                            <ChevronRightIcon
                                class="size-5 shrink-0 text-[var(--color-text-tertiary)]"
                                aria-hidden="true"
                            />
                        </button>
                    </li>
                </ul>
            </template>

            <template v-else>
                <div class="flex min-w-0 items-center gap-3">
                    <Button
                        v-tooltip="formatMessage(messages.back)"
                        type="quiet"
                        size="2xs"
                        circular
                        icon-only
                        :aria-label="formatMessage(messages.back)"
                        @click="backToInstances"
                        ><ChevronLeftIcon />
                    </Button>
                    <InstanceIcon
                        class="size-10 shrink-0"
                        :icon-path="selectedInstance.icon_path"
                        :instance-id="selectedInstance.id"
                        :loader="selectedInstance.loader"
                    />
                    <div class="flex min-w-0 flex-1 flex-col">
                        <strong class="truncate text-[var(--color-text-primary)]">{{
                            selectedInstance.name
                        }}</strong>
                        <span class="truncate text-sm capitalize text-[var(--color-text-tertiary)]">
                            {{ selectedInstance.loader }} {{ selectedInstance.game_version }}
                        </span>
                    </div>
                </div>

                <StyledInput
                    ref="searchInput"
                    v-model="search"
                    :icon="SearchIcon"
                    type="search"
                    :placeholder="formatMessage(messages.searchSchematics)"
                    wrapper-class="w-full"
                    clearable
                />

                <div
                    v-if="loadingFiles"
                    class="flex flex-1 items-center justify-center text-[var(--color-text-tertiary)]"
                >
                    <SpinnerIcon class="size-6 animate-spin" />
                </div>
                <p
                    v-else-if="error"
                    class="m-0 flex flex-1 items-center justify-center text-center text-brand-red"
                >
                    {{ error }}
                </p>
                <p
                    v-else-if="files.length === 0"
                    class="m-0 flex flex-1 items-center justify-center text-center text-[var(--color-text-tertiary)]"
                >
                    {{ formatMessage(messages.noSchematics) }}
                </p>
                <p
                    v-else-if="visibleRows.length === 0"
                    class="m-0 flex flex-1 items-center justify-center text-center text-[var(--color-text-tertiary)]"
                >
                    {{ formatMessage(messages.noMatchingSchematics) }}
                </p>
                <div v-else class="max-h-[30rem] overflow-y-auto pr-1">
                    <div
                        ref="listContainer"
                        role="list"
                        class="relative"
                        :style="{ height: `${totalHeight}px`, overflowAnchor: 'none' }"
                    >
                        <div class="absolute inset-x-0" :style="{ top: `${visibleTop}px` }">
                            <template v-for="row in visibleItems" :key="rowKey(row)">
                                <button
                                    v-if="row.kind === 'folder'"
                                    v-tooltip="row.path"
                                    type="button"
                                    role="listitem"
                                    class="flex h-16 w-full cursor-pointer items-center gap-3 border-0 border-b border-solid border-surface-5 bg-transparent py-2 pr-3 text-left text-[var(--color-text-default)] transition-colors last:border-b-0 hover:bg-surface-4 focus-visible:outline-none focus-visible:ring-4 focus-visible:ring-brand-shadow"
                                    :style="rowPadding(row.depth)"
                                    :aria-expanded="row.expanded"
                                    :aria-label="
                                        formatMessage(
                                            row.expanded
                                                ? messages.collapseFolder
                                                : messages.expandFolder,
                                            {
                                                name: row.name,
                                            },
                                        )
                                    "
                                    @click="toggleFolder(row.path)"
                                >
                                    <ChevronDownIcon
                                        v-if="row.expanded"
                                        class="size-5 shrink-0 text-[var(--color-text-tertiary)]"
                                        aria-hidden="true"
                                    />
                                    <ChevronRightIcon
                                        v-else
                                        class="size-5 shrink-0 text-[var(--color-text-tertiary)]"
                                        aria-hidden="true"
                                    />
                                    <FolderIcon
                                        class="size-5 shrink-0 text-[var(--color-text-tertiary)]"
                                        aria-hidden="true"
                                    />
                                    <span class="flex min-w-0 flex-1 items-baseline gap-1.5">
                                        <strong class="truncate text-[var(--color-text-primary)]">{{
                                            row.name
                                        }}</strong>
                                        <span
                                            class="shrink-0 text-sm font-medium text-[var(--color-text-tertiary)]"
                                        >
                                            ({{ row.fileCount }})
                                        </span>
                                    </span>
                                </button>
                                <button
                                    v-else
                                    v-tooltip="row.file.relativePath"
                                    type="button"
                                    role="listitem"
                                    class="flex h-16 w-full cursor-pointer items-center gap-3 border-0 border-b border-solid border-surface-5 bg-transparent py-2 pr-3 text-left text-[var(--color-text-default)] transition-colors last:border-b-0 hover:bg-surface-4 focus-visible:outline-none focus-visible:ring-4 focus-visible:ring-brand-shadow"
                                    :style="rowPadding(row.depth)"
                                    :aria-label="
                                        formatMessage(messages.openSchematic, {
                                            name: row.file.relativePath,
                                        })
                                    "
                                    @click="openFile(row.file)"
                                >
                                    <FileArchiveIcon
                                        class="size-5 shrink-0 text-[var(--color-text-tertiary)]"
                                    />
                                    <span class="flex min-w-0 flex-1 flex-col gap-0.5">
                                        <strong class="truncate text-[var(--color-text-primary)]">{{
                                            row.file.fileName
                                        }}</strong>
                                        <span
                                            class="truncate text-xs uppercase text-[var(--color-text-tertiary)]"
                                        >
                                            <span v-if="row.parentPath"
                                                >{{ row.parentPath }} · </span
                                            >{{ row.file.format }} ·
                                            {{ formatBytes(row.file.size) }}
                                            <span v-if="row.file.modifiedAt">
                                                · {{ formatModified(row.file.modifiedAt) }}</span
                                            >
                                        </span>
                                    </span>
                                    <ChevronRightIcon
                                        class="size-5 shrink-0 text-[var(--color-text-tertiary)]"
                                        aria-hidden="true"
                                    />
                                </button>
                            </template>
                        </div>
                    </div>
                </div>
            </template>
        </div>
    </NewModal>
</template>
