<script setup lang="ts">
import { CheckIcon, ChevronDownIcon, ExternalIcon, PlusIcon, SpinnerIcon } from '@modrinth/assets'
import { Button, injectNotificationManager, PopoutMenu, useVIntl } from '@modrinth/ui'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { openUrl } from '@tauri-apps/plugin-opener'
import { computed } from 'vue'

import { useAppEvent } from '@/composables/use-app-event'
import { get_content_snapshot } from '@/helpers/instance'
import {
    hasInstalledSchematic,
    installedSchematicsKey,
    installSchematic,
    type SchematicProject,
} from '@/helpers/schematics'

import { schematicMessages as messages } from './messages'

const props = defineProps<{
    project: SchematicProject
    instanceId: string | null
    compact?: boolean
    prominent?: boolean
    fileName?: string
}>()
const emit = defineEmits<{ chooseInstance: []; authorizationError: [] }>()
const { formatMessage } = useVIntl()
const { addNotification, handleError } = injectNotificationManager()
const files = computed(() =>
    props.fileName
        ? props.project.files.filter((file) => file.name === props.fileName)
        : props.project.files,
)
const queryClient = useQueryClient()
const installedQuery = useQuery({
    queryKey: computed(() => installedSchematicsKey(props.instanceId)),
    queryFn: () => get_content_snapshot(props.instanceId!, true, true),
    enabled: computed(() => !!props.instanceId),
    retry: false,
})
const checkingInstalled = computed(() => !!props.instanceId && installedQuery.isPending.value)
const isInstalled = (fileName: string) =>
    hasInstalledSchematic(
        installedQuery.data.value,
        props.project.source,
        props.project.id,
        fileName,
    )
useAppEvent('instance', (event) => {
    if (
        event.instance_id === props.instanceId &&
        ['synced', 'content_changed', 'removed'].includes(event.event)
    )
        void queryClient.invalidateQueries(
            { queryKey: installedSchematicsKey(props.instanceId) },
            { cancelRefetch: false },
        )
})
const installation = useMutation({
    mutationFn: async (request: Parameters<typeof installSchematic>[0]) => {
        const snapshot = await queryClient.fetchQuery({
            queryKey: installedSchematicsKey(request.instanceId),
            queryFn: () => get_content_snapshot(request.instanceId, true, true),
            staleTime: 0,
        })
        if (hasInstalledSchematic(snapshot, request.source, request.id, request.fileName))
            return null
        return installSchematic(request)
    },
    onSuccess: (path, request) => {
        void queryClient.invalidateQueries({
            queryKey: installedSchematicsKey(request.instanceId),
        })
        if (!path) return
        addNotification({
            title: formatMessage(messages.installSuccess),
            text: path.split(/[\\/]/).pop() ?? request.fileName,
            type: 'success',
        })
    },
    onError: (error, request) => {
        const message =
            typeof error === 'object' && error && 'message' in error
                ? String(error.message)
                : String(error)
        addNotification({
            title: formatMessage(messages.installFailed),
            text: message.includes('schematic_target_exists')
                ? formatMessage(messages.fileExists)
                : '',
            type: 'error',
        })
        if (request.source === 'shimu') emit('authorizationError')
    },
})
const busy = installation.isPending
const fileKey = computed(() => installation.variables.value?.fileName)
const allInstalled = computed(() =>
    files.value.length ? files.value.every((file) => isInstalled(file.name)) : isInstalled(''),
)
const installLabel = computed(() =>
    formatMessage(
        allInstalled.value
            ? messages.installed
            : busy.value
              ? messages.installing
              : messages.install,
    ),
)
function install(fileName: string) {
    if (!props.instanceId) {
        emit('chooseInstance')
        return
    }
    installation.mutate({
        instanceId: props.instanceId,
        source: props.project.source,
        id: props.project.id,
        fileName,
    })
}
</script>

<template>
    <Button
        v-if="project.filesResolved && !project.files.length"
        type="outlined"
        @click="openUrl(project.sourceUrl).catch(handleError)"
    >
        <ExternalIcon />{{ formatMessage(messages.openSource) }}
    </Button>
    <Button
        v-else-if="files.length <= 1 || !instanceId"
        color="brand"
        :type="prominent ? 'colored' : 'outlined'"
        :size="compact ? '2xs' : prominent ? 'xl' : 'md'"
        :icon-only="compact"
        :circular="compact"
        :label="installLabel"
        :title="installLabel"
        :disabled="busy || checkingInstalled || allInstalled"
        @click.stop="install(files[0]?.name ?? '')"
    >
        <SpinnerIcon v-if="busy" class="animate-spin motion-reduce:animate-none" />
        <CheckIcon v-else-if="allInstalled" />
        <PlusIcon v-else />
        <template v-if="!compact">{{ installLabel }}</template>
    </Button>
    <PopoutMenu v-else placement="bottom-end">
        <template #trigger>
            <Button
                color="brand"
                :type="prominent ? 'colored' : 'outlined'"
                :size="compact ? '2xs' : prominent ? 'xl' : 'md'"
                :icon-only="compact"
                :circular="compact"
                :label="installLabel"
                :title="installLabel"
                :disabled="busy || checkingInstalled || allInstalled"
            >
                <SpinnerIcon v-if="busy" class="animate-spin motion-reduce:animate-none" />
                <CheckIcon v-else-if="allInstalled" />
                <PlusIcon v-else />
                <template v-if="!compact">{{ installLabel }}<ChevronDownIcon /></template>
            </Button>
        </template>
        <template #menu>
            <div class="flex max-w-sm flex-col gap-1 p-1">
                <Button
                    v-for="file in files"
                    :key="file.name"
                    type="quiet"
                    class="w-full !justify-start text-left"
                    :disabled="busy || isInstalled(file.name)"
                    @click.stop="install(file.name)"
                >
                    <SpinnerIcon
                        v-if="busy && fileKey === file.name"
                        class="shrink-0 animate-spin motion-reduce:animate-none"
                    />
                    <CheckIcon v-else-if="isInstalled(file.name)" class="shrink-0" />
                    <PlusIcon v-else class="shrink-0" />
                    <span class="min-w-0 break-all">{{ file.name }}</span>
                </Button>
            </div>
        </template>
    </PopoutMenu>
</template>
