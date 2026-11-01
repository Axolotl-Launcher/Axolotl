<script setup lang="ts">
import { CheckIcon, ChevronDownIcon, PlusIcon, SpinnerIcon } from '@modrinth/assets'
import { Button, injectNotificationManager, PopoutMenu, useVIntl } from '@modrinth/ui'
import { useMutation } from '@tanstack/vue-query'
import { computed, ref } from 'vue'

import { installShimuSchematic, type ShimuSchematic } from '@/helpers/shimu-schematics'

import { schematicMessages as messages } from './messages'

const props = defineProps<{
    project: ShimuSchematic
    instanceId: string | null
    compact?: boolean
    prominent?: boolean
    fileName?: string
}>()
const emit = defineEmits<{ chooseInstance: []; authorizationError: [] }>()
const { formatMessage } = useVIntl()
const { addNotification } = injectNotificationManager()
const files = computed(() =>
    props.fileName
        ? props.project.files.filter((file) => file.name === props.fileName)
        : props.project.files,
)
const installed = ref(new Set<string>())
const identity = (fileName: string) =>
    `${props.instanceId}\u0000shimu\u0000${props.project.postId}\u0000${fileName}`
const installation = useMutation({
    mutationFn: installShimuSchematic,
    onSuccess: (_, request) => {
        installed.value.add(
            `${request.instanceId}\u0000shimu\u0000${request.postId}\u0000${request.fileName}`,
        )
        addNotification({
            title: formatMessage(messages.installSuccess),
            text: request.fileName,
            type: 'success',
        })
    },
    onError: (error) => {
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
        emit('authorizationError')
    },
})
const busy = installation.isPending
const fileKey = computed(() => installation.variables.value?.fileName)
const allInstalled = computed(() =>
    files.value.every((file) => installed.value.has(identity(file.name))),
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
    installation.mutate({ instanceId: props.instanceId, postId: props.project.postId, fileName })
}
</script>

<template>
    <Button
        v-if="files.length === 1 || !instanceId"
        color="brand"
        :type="prominent ? 'colored' : 'outlined'"
        :size="compact ? '2xs' : prominent ? 'xl' : 'md'"
        :icon-only="compact"
        :circular="compact"
        :label="installLabel"
        :title="installLabel"
        :disabled="busy || allInstalled"
        @click.stop="install(files[0].name)"
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
                :disabled="busy || allInstalled"
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
                    :disabled="busy || installed.has(identity(file.name))"
                    @click.stop="install(file.name)"
                >
                    <SpinnerIcon
                        v-if="busy && fileKey === file.name"
                        class="shrink-0 animate-spin motion-reduce:animate-none"
                    />
                    <CheckIcon v-else-if="installed.has(identity(file.name))" class="shrink-0" />
                    <PlusIcon v-else class="shrink-0" />
                    <span class="min-w-0 break-all">{{ file.name }}</span>
                </Button>
            </div>
        </template>
    </PopoutMenu>
</template>
