<script setup lang="ts">
import CheckIcon from '@modrinth/assets/icons/check.svg?component'
import DownloadIcon from '@modrinth/assets/icons/download.svg?component'
import PlusIcon from '@modrinth/assets/icons/plus.svg?component'
import SearchIcon from '@modrinth/assets/icons/search.svg?component'
import TrashIcon from '@modrinth/assets/icons/trash.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import EmptyState from '@modrinth/ui/src/components/base/EmptyState.vue'
import StyledInput from '@modrinth/ui/src/components/base/StyledInput.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { computed, nextTick, ref } from 'vue'
import { useRouter } from 'vue-router'

import type { GameInstance } from '../../../lib/types'
import PreviewModal from '../../../overlays/PreviewModal.vue'
import InstanceIcon from '../../../shell/InstanceIcon.vue'
import { messages } from '../messages'

/**
 * Port of `BrowseInstanceSelector` (apps/app-frontend/src/components/browse/
 * BrowseInstanceSelector.vue) with its `InstancePickerList`: "Choose an instance" and the
 * "Switch installation target?" prompt when the cart already holds projects.
 */
const props = defineProps<{
    instances: GameInstance[]
    selectedInstance: GameInstance | null
    selectedCount: number
    installCurrent: () => Promise<boolean>
    clearCurrent: () => void
}>()

const emit = defineEmits<{
    select: [instance: GameInstance]
    cancelSwitch: []
}>()

const { formatMessage, locale } = useVIntl()
const router = useRouter()
const pickerModal = ref<InstanceType<typeof PreviewModal>>()
const switchModal = ref<InstanceType<typeof PreviewModal>>()
const searchInput = ref<InstanceType<typeof StyledInput>>()
const searchQuery = ref('')
const pendingInstance = ref<GameInstance | null>(null)
const installingCurrent = ref(false)

const visibleInstances = computed(() => {
    const query = searchQuery.value.trim().toLocaleLowerCase(locale.value)
    return props.instances
        .filter((instance) => {
            if (!query) return true
            return [instance.name, instance.loader, instance.game_version].some((value) =>
                value.toLocaleLowerCase(locale.value).includes(query),
            )
        })
        .slice()
        .sort((a, b) => a.name.localeCompare(b.name, locale.value, { sensitivity: 'base' }))
})

function show() {
    searchQuery.value = ''
    pickerModal.value?.show()
    void nextTick(() => (searchInput.value as { focus?: () => void } | undefined)?.focus?.())
}

function choose(instance: GameInstance) {
    if (instance.id === props.selectedInstance?.id) {
        pickerModal.value?.hide()
        return
    }
    if (props.selectedCount > 0 && props.selectedInstance) {
        pendingInstance.value = instance
        pickerModal.value?.hide()
        void nextTick(() => switchModal.value?.show())
        return
    }
    emit('select', instance)
    pickerModal.value?.hide()
}

function requestSwitch(instance: GameInstance) {
    choose(instance)
}

async function installAndSwitch() {
    if (!pendingInstance.value || installingCurrent.value) return
    installingCurrent.value = true
    try {
        if (!(await props.installCurrent())) return
        const next = pendingInstance.value
        pendingInstance.value = null
        installingCurrent.value = false
        switchModal.value?.hide()
        if (next) emit('select', next)
    } finally {
        installingCurrent.value = false
    }
}

function clearAndSwitch() {
    if (!pendingInstance.value) return
    props.clearCurrent()
    const next = pendingInstance.value
    pendingInstance.value = null
    switchModal.value?.hide()
    emit('select', next)
}

function createInstance() {
    pickerModal.value?.hide()
    void router.push('/create')
}

function cancelSwitch() {
    pendingInstance.value = null
    switchModal.value?.hide()
    emit('cancelSwitch')
}

defineExpose({ show, requestSwitch })
</script>

<template>
    <PreviewModal
        ref="pickerModal"
        :header="formatMessage(messages.instanceSelectorTitle)"
        max-width="560px"
        width="min(560px, calc(1280px - 2rem))"
        scrollable
        max-content-height="min(28rem, calc(800px - 18rem))"
        actions-divider
    >
        <div class="flex min-w-0 flex-col">
            <StyledInput
                v-if="instances.length > 0"
                ref="searchInput"
                v-model="searchQuery"
                type="search"
                :icon="SearchIcon"
                :placeholder="formatMessage(messages.instanceSelectorSearch)"
                wrapper-class="w-full"
                clearable
            />
            <ul v-if="visibleInstances.length > 0" class="m-0 flex list-none flex-col gap-1 p-0">
                <li v-for="instance in visibleInstances" :key="instance.id" class="min-w-0">
                    <button
                        type="button"
                        class="flex min-h-16 w-full cursor-pointer items-center gap-3 rounded-lg border-0 bg-transparent px-3 py-2 text-left text-[var(--color-text-default)] transition-colors hover:bg-surface-4 focus-visible:outline-none focus-visible:ring-4 focus-visible:ring-brand-shadow"
                        :aria-label="
                            formatMessage(messages.instanceSelectorSelect, { name: instance.name })
                        "
                        @click="choose(instance)"
                    >
                        <InstanceIcon
                            class="shrink-0"
                            size="2.5rem"
                            :icon-path="instance.icon_path"
                            :instance-id="instance.id"
                            :loader="instance.loader"
                        />
                        <span class="flex min-w-0 flex-1 flex-col gap-0.5">
                            <span class="truncate font-semibold text-[var(--color-text-primary)]">{{
                                instance.name
                            }}</span>
                            <span
                                class="truncate text-sm capitalize text-[var(--color-text-tertiary)]"
                            >
                                {{ instance.loader }} {{ instance.game_version }}
                            </span>
                        </span>
                        <CheckIcon
                            v-if="instance.id === selectedInstance?.id"
                            class="size-5 shrink-0 text-brand"
                            aria-hidden="true"
                        />
                    </button>
                </li>
            </ul>
            <EmptyState
                v-else
                type="empty-inbox"
                :heading="
                    instances.length === 0
                        ? formatMessage(messages.instanceSelectorNoInstances)
                        : formatMessage(messages.instanceSelectorNoResults)
                "
            />
        </div>
        <template #actions>
            <div class="flex justify-start">
                <Button type="quiet" @click="createInstance"
                    ><PlusIcon />
                    {{ formatMessage(messages.instanceSelectorCreate) }}
                </Button>
            </div>
        </template>
    </PreviewModal>

    <PreviewModal
        ref="switchModal"
        :header="formatMessage(messages.switchTargetTitle)"
        width="min(600px, calc(1280px - 2rem))"
        max-width="600px"
        scrollable
        max-content-height="min(32rem, 560px)"
        actions-divider
        :disable-close="installingCurrent"
    >
        <div v-if="pendingInstance && selectedInstance" class="flex min-w-0 flex-col gap-5">
            <div
                class="grid min-w-0 gap-3 sm:grid-cols-[minmax(0,1fr)_1px_minmax(0,1fr)] sm:items-stretch"
            >
                <div class="flex min-w-0 flex-col gap-2 px-1 py-1">
                    <span class="text-xs font-semibold text-[var(--color-text-tertiary)]">
                        {{ formatMessage(messages.currentTarget) }}
                    </span>
                    <div class="flex min-w-0 items-center gap-3">
                        <InstanceIcon
                            class="shrink-0"
                            size="2.5rem"
                            :icon-path="selectedInstance.icon_path"
                            :instance-id="selectedInstance.id"
                            :loader="selectedInstance.loader"
                        />
                        <span class="flex min-w-0 flex-1 flex-col gap-0.5">
                            <span class="truncate font-semibold text-[var(--color-text-primary)]">{{
                                selectedInstance.name
                            }}</span>
                            <span
                                class="truncate text-sm capitalize text-[var(--color-text-tertiary)]"
                            >
                                {{ selectedInstance.loader }} {{ selectedInstance.game_version }}
                            </span>
                        </span>
                    </div>
                </div>

                <div aria-hidden="true" class="h-px w-full bg-surface-4 sm:h-auto sm:w-px" />

                <div class="flex min-w-0 flex-col gap-2 px-1 py-1">
                    <span class="text-xs font-semibold text-[var(--color-text-tertiary)]">
                        {{ formatMessage(messages.newTarget) }}
                    </span>
                    <div class="flex min-w-0 items-center gap-3">
                        <InstanceIcon
                            class="shrink-0"
                            size="2.5rem"
                            :icon-path="pendingInstance.icon_path"
                            :instance-id="pendingInstance.id"
                            :loader="pendingInstance.loader"
                        />
                        <span class="flex min-w-0 flex-1 flex-col gap-0.5">
                            <span class="truncate font-semibold text-[var(--color-text-primary)]">{{
                                pendingInstance.name
                            }}</span>
                            <span
                                class="truncate text-sm capitalize text-[var(--color-text-tertiary)]"
                            >
                                {{ pendingInstance.loader }} {{ pendingInstance.game_version }}
                            </span>
                        </span>
                    </div>
                </div>
            </div>

            <p class="m-0 text-sm text-[var(--color-text-tertiary)]">
                {{ formatMessage(messages.switchTargetDescription, { count: selectedCount }) }}
            </p>
        </div>
        <template #actions>
            <div class="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-end">
                <Button
                    type="outlined"
                    class="w-full sm:w-auto"
                    :disabled="installingCurrent"
                    @click="cancelSwitch"
                    >{{ formatMessage(messages.instanceSelectorCancel) }}
                </Button>
                <Button
                    type="outlined"
                    color="red"
                    class="w-full sm:w-auto"
                    :disabled="installingCurrent"
                    @click="clearAndSwitch"
                    ><TrashIcon />
                    {{ formatMessage(messages.clearAndSwitch) }}
                </Button>
                <Button
                    type="colored"
                    color="brand"
                    class="w-full sm:w-auto"
                    :disabled="installingCurrent"
                    @click="installAndSwitch"
                    ><DownloadIcon />
                    {{
                        formatMessage(
                            installingCurrent
                                ? messages.installingCurrent
                                : messages.installCurrent,
                        )
                    }}
                </Button>
            </div>
        </template>
    </PreviewModal>
</template>
