<script setup lang="ts">
import ChevronRightIcon from '@modrinth/assets/icons/chevron-right.svg?component'
import CodeIcon from '@modrinth/assets/icons/code.svg?component'
import CoffeeIcon from '@modrinth/assets/icons/coffee.svg?component'
import FileArchiveIcon from '@modrinth/assets/icons/file-archive.svg?component'
import InfoIcon from '@modrinth/assets/icons/info.svg?component'
import MonitorIcon from '@modrinth/assets/icons/monitor.svg?component'
import WrenchIcon from '@modrinth/assets/icons/wrench.svg?component'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { commonMessages } from '@modrinth/ui/src/utils/common-messages.ts'
import { computed, defineAsyncComponent, nextTick, ref, watch } from 'vue'

import { isServerProjectLink } from '../../../lib/instance-project'
import type { GameInstance } from '../../../lib/types'
import InstanceIcon from '../../../shell/InstanceIcon.vue'
import { settingsTabMessages } from '../messages'
import { provideInstanceSettings } from './context'
import TabbedModal, { type Tab } from './TabbedModal.vue'

/**
 * Port of `apps/app-frontend/src/components/ui/modal/InstanceSettingsModal.vue`.
 * The tab bodies are loaded with the modal's first use so the instance page's
 * chunk stays small; `TabbedModal` already renders them inside `<Suspense>`.
 */
const GeneralSettings = defineAsyncComponent(() => import('./GeneralSettings.vue'))
const InstallationSettings = defineAsyncComponent(() => import('./InstallationSettings.vue'))
const SyncSettings = defineAsyncComponent(() => import('./SyncSettings.vue'))
const BackupSettings = defineAsyncComponent(() => import('./BackupSettings.vue'))
const CoreComponentsSettings = defineAsyncComponent(() => import('./CoreComponentsSettings.vue'))
const WindowSettings = defineAsyncComponent(() => import('./WindowSettings.vue'))
const JavaSettings = defineAsyncComponent(() => import('./JavaSettings.vue'))
const HooksSettings = defineAsyncComponent(() => import('./HooksSettings.vue'))

const { formatMessage } = useVIntl()

const props = defineProps<{
    instance: GameInstance
    offline?: boolean
}>()
const emit = defineEmits<{
    unlinked: []
    updated: [instance: GameInstance]
}>()

/** Snapshot server instances carry explicit server-project links. */
const isMinecraftServer = ref(false)
watch(
    () => props.instance,
    (instance) => {
        isMinecraftServer.value = isServerProjectLink(instance.link)
    },
    { immediate: true },
)
const handleUnlinked = () => emit('unlinked')

const instanceRef = computed(() => props.instance)
const tabbedModal = ref<InstanceType<typeof TabbedModal> | null>(null)

function hide() {
    tabbedModal.value?.hide()
}

provideInstanceSettings({
    instance: instanceRef,
    offline: props.offline,
    isMinecraftServer,
    onUnlinked: handleUnlinked,
    onInstanceUpdated: (instance) => emit('updated', instance),
    closeModal: hide,
})

const tabs = computed<Tab[]>(() => [
    {
        name: settingsTabMessages.tabsGeneral,
        icon: InfoIcon,
        content: GeneralSettings,
    },
    {
        name: settingsTabMessages.tabsInstallation,
        icon: WrenchIcon,
        content: InstallationSettings,
    },
    {
        name: settingsTabMessages.tabsSync,
        icon: WrenchIcon,
        content: SyncSettings,
    },
    {
        name: settingsTabMessages.tabsBackups,
        icon: FileArchiveIcon,
        content: BackupSettings,
    },
    {
        name: settingsTabMessages.tabsCoreComponents,
        icon: FileArchiveIcon,
        content: CoreComponentsSettings,
    },
    {
        name: settingsTabMessages.tabsWindow,
        icon: MonitorIcon,
        content: WindowSettings,
    },
    {
        name: settingsTabMessages.tabsJava,
        icon: CoffeeIcon,
        content: JavaSettings,
    },
    {
        name: settingsTabMessages.tabsHooks,
        icon: CodeIcon,
        content: HooksSettings,
    },
])

function show(tabIndex?: number) {
    tabbedModal.value?.show()
    if (tabIndex !== undefined) {
        nextTick(() => tabbedModal.value?.setTab(tabIndex))
    }
}

defineExpose({ show, hide })
</script>

<template>
    <TabbedModal ref="tabbedModal" :tabs="tabs" max-width="928px" width="928px">
        <template #title>
            <span
                class="flex items-center gap-2 text-lg font-semibold text-[var(--color-text-default)]"
            >
                <InstanceIcon
                    :icon-path="instance.icon_path"
                    :instance-id="props.instance.id"
                    :loader="instance.loader"
                    size="24px"
                />
                {{ instance.name }} <ChevronRightIcon />
                <span class="font-extrabold text-[var(--color-text-primary)]">{{
                    formatMessage(commonMessages.settingsLabel)
                }}</span>
            </span>
        </template>
    </TabbedModal>
</template>
