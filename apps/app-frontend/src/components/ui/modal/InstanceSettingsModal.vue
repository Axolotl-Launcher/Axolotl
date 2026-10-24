<script setup lang="ts">
import type { Labrinth } from '@modrinth/api-client'
import {
    ChevronRightIcon,
    CodeIcon,
    CoffeeIcon,
    FileArchiveIcon,
    InfoIcon,
    MonitorIcon,
    WrenchIcon,
} from '@modrinth/assets'
import {
    commonMessages,
    defineMessage,
    TabbedModal,
    type TabbedModalTab,
    useVIntl,
} from '@modrinth/ui'
import type { PlatformTag } from '@modrinth/utils'
import { useQuery, useQueryClient } from '@tanstack/vue-query'
import { computed, nextTick, ref, watch } from 'vue'

import BackupSettings from '@/components/ui/instance_settings/BackupSettings.vue'
import CoreComponentsSettings from '@/components/ui/instance_settings/CoreComponentsSettings.vue'
import GeneralSettings from '@/components/ui/instance_settings/GeneralSettings.vue'
import HooksSettings from '@/components/ui/instance_settings/HooksSettings.vue'
import InstallationSettings from '@/components/ui/instance_settings/InstallationSettings.vue'
import JavaSettings from '@/components/ui/instance_settings/JavaSettings.vue'
import SyncSettings from '@/components/ui/instance_settings/SyncSettings.vue'
import WindowSettings from '@/components/ui/instance_settings/WindowSettings.vue'
import InstanceIcon from '@/components/ui/InstanceIcon.vue'
import { get_project_v3 } from '@/helpers/cache'
import { get_linked_modpack_info } from '@/helpers/instance'
import { instanceServerProjectId, isServerProjectLink } from '@/helpers/instance-project'
import { get_game_versions, get_loaders } from '@/helpers/tags'
import { provideInstanceSettings } from '@/providers/instance-settings'

import type { GameInstance } from '../../../helpers/types'

const { formatMessage } = useVIntl()
const queryClient = useQueryClient()

const props = defineProps<{
    instance: GameInstance
    offline?: boolean
}>()
const emit = defineEmits<{
    unlinked: []
    updated: [instance: GameInstance]
}>()

const isMinecraftServer = ref(false)
const handleUnlinked = () => emit('unlinked')
let serverMetadataGeneration = 0

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

watch(
    () => props.instance,
    (instance) => {
        const generation = ++serverMetadataGeneration
        isMinecraftServer.value = isServerProjectLink(instance.link)
        const projectId = instanceServerProjectId(instance.link)
        if (instance.install_stage === 'installed' && projectId) {
            const instanceId = instance.id
            get_project_v3(projectId, 'must_revalidate')
                .then((project: Labrinth.Projects.v3.Project | undefined) => {
                    if (
                        generation === serverMetadataGeneration &&
                        props.instance.id === instanceId &&
                        props.instance.install_stage === 'installed' &&
                        project?.minecraft_server != null
                    ) {
                        isMinecraftServer.value = true
                    }
                })
                .catch(() => {})
        }
    },
    { immediate: true },
)

const tabs = computed<TabbedModalTab[]>(() => [
    {
        name: defineMessage({
            id: 'instance.settings.tabs.general',
            defaultMessage: 'General',
        }),
        icon: InfoIcon,
        content: GeneralSettings,
    },
    {
        name: defineMessage({
            id: 'instance.settings.tabs.installation',
            defaultMessage: 'Installation',
        }),
        icon: WrenchIcon,
        content: InstallationSettings,
    },
    {
        name: defineMessage({
            id: 'instance.settings.tabs.sync',
            defaultMessage: 'Synchronization',
        }),
        icon: WrenchIcon,
        content: SyncSettings,
    },
    {
        name: defineMessage({
            id: 'instance.settings.tabs.backups',
            defaultMessage: 'Backups',
        }),
        icon: FileArchiveIcon,
        content: BackupSettings,
    },
    {
        // Core component editing is instance-specific and advanced, so it is intentionally excluded from first-run onboarding.
        name: defineMessage({
            id: 'instance.settings.tabs.core-components',
            defaultMessage: 'Core components',
        }),
        icon: FileArchiveIcon,
        content: CoreComponentsSettings,
    },
    {
        name: defineMessage({
            id: 'instance.settings.tabs.window',
            defaultMessage: 'Window',
        }),
        icon: MonitorIcon,
        content: WindowSettings,
    },
    {
        name: defineMessage({
            id: 'instance.settings.tabs.java',
            defaultMessage: 'Java and memory',
        }),
        icon: CoffeeIcon,
        content: JavaSettings,
    },
    {
        name: defineMessage({
            id: 'instance.settings.tabs.hooks',
            defaultMessage: 'Launch preparation',
        }),
        icon: CodeIcon,
        content: HooksSettings,
    },
])

function getSupportedModpackLoaders() {
    return get_loaders().then((value: PlatformTag[]) =>
        value
            .filter(
                (item) =>
                    item.supported_project_types.includes('modpack') || item.name === 'vanilla',
            )
            .sort((a, b) => (a.name === 'vanilla' ? -1 : b.name === 'vanilla' ? 1 : 0)),
    )
}

// Preload metadata that is not scoped to an editable Minecraft version.
useQuery({
    queryKey: ['instance-settings', 'game-versions'],
    queryFn: get_game_versions,
})
useQuery({
    queryKey: ['instance-settings', 'loaders', 'modpack'],
    queryFn: getSupportedModpackLoaders,
})
useQuery({
    queryKey: computed(() => ['linkedModpackInfo', props.instance.id]),
    queryFn: () => get_linked_modpack_info(props.instance.id, 'stale_while_revalidate'),
    enabled: computed(
        () =>
            props.instance.install_stage === 'installed' &&
            !!props.instance.link?.project_id &&
            !props.offline,
    ),
})

function show(tabIndex?: number) {
    if (props.instance.install_stage === 'installed' && props.instance.link?.project_id) {
        queryClient.prefetchQuery({
            queryKey: ['linkedModpackInfo', props.instance.id],
            queryFn: () => get_linked_modpack_info(props.instance.id, 'stale_while_revalidate'),
        })
    }
    tabbedModal.value?.show()
    if (tabIndex !== undefined) {
        nextTick(() => tabbedModal.value?.setTab(tabIndex))
    }
}

defineExpose({ show, hide })
</script>
<template>
    <TabbedModal
        ref="tabbedModal"
        :tabs="tabs"
        :max-width="'min(928px, calc(95vw - 10rem))'"
        :width="'min(928px, calc(95vw - 10rem))'"
    >
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
