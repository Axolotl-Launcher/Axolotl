<script setup lang="ts">
import { CompassIcon, RefreshCwIcon } from '@modrinth/assets'

import Button from '#ui/components/base/buttons/Button.vue'
import EmptyState from '#ui/components/base/EmptyState.vue'
import { defineMessages, useVIntl } from '#ui/composables/i18n'
import { commonMessages, formatContentTypeSentence } from '#ui/utils/common-messages'

const { formatMessage } = useVIntl()

const messages = defineMessages({
    noContentInstalled: {
        id: 'content.page-layout.empty.no-content-installed',
        defaultMessage: 'No content installed',
    },
    emptyHint: {
        id: 'content.page-layout.empty.hint',
        defaultMessage: 'Browse or upload {contentType} to get started',
    },
    browseContent: {
        id: 'content.page-layout.browse-content',
        defaultMessage: 'Browse content',
    },
})

const props = withDefaults(
    defineProps<{
        contentTypeLabel: string
        busy?: boolean
        busyTooltip?: string | null
        refreshing?: boolean
        disableAddContent?: boolean
        disableAddContentTooltip?: string
    }>(),
    {
        busy: false,
        busyTooltip: null,
        refreshing: false,
        disableAddContent: false,
        disableAddContentTooltip: undefined,
    },
)

const emit = defineEmits<{
    browse: []
    refresh: []
}>()
</script>

<template>
    <EmptyState type="empty-inbox">
        <template #heading>
            {{ formatMessage(messages.noContentInstalled) }}
        </template>
        <template #description>
            {{
                formatMessage(messages.emptyHint, {
                    contentType: formatContentTypeSentence(
                        formatMessage,
                        props.contentTypeLabel,
                        2,
                        'content',
                    ),
                })
            }}
        </template>
        <template #actions>
            <slot name="actions" />
            <Button
                v-tooltip="props.busyTooltip"
                type="outlined"
                :disabled="props.refreshing"
                class="!h-10"
                @click="emit('refresh')"
                ><RefreshCwIcon :class="['size-5', { 'animate-spin': props.refreshing }]" />
                {{ formatMessage(commonMessages.refreshButton) }}
            </Button>
            <Button
                v-tooltip="
                    props.busyTooltip ??
                    (props.disableAddContent ? props.disableAddContentTooltip : undefined)
                "
                type="colored"
                color="brand"
                :disabled="props.busy || props.disableAddContent"
                class="!h-10 flex items-center gap-2"
                @click="emit('browse')"
                ><CompassIcon class="size-5" />
                <span>{{ formatMessage(messages.browseContent) }}</span>
            </Button>
        </template>
    </EmptyState>
</template>
