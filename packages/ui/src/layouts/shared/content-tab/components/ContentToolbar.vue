<script setup lang="ts">
import { CompassIcon, GitGraphIcon, RefreshCwIcon, SearchIcon } from '@modrinth/assets'

import Button from '#ui/components/base/buttons/Button.vue'
import StyledInput from '#ui/components/base/StyledInput.vue'
import { defineMessages, useVIntl } from '#ui/composables/i18n'
import { commonMessages, formatContentTypeSentence } from '#ui/utils/common-messages'

const { formatMessage } = useVIntl()

const messages = defineMessages({
    searchPlaceholder: {
        id: 'content.page-layout.search-placeholder',
        defaultMessage: 'Search {count, number} {contentType}...',
    },
    browseContent: {
        id: 'content.page-layout.browse-content',
        defaultMessage: 'Browse content',
    },
    viewDependencies: {
        id: 'content.page-layout.view-dependencies',
        defaultMessage: 'View dependencies',
    },
})

const searchQuery = defineModel<string>('searchQuery', { required: true })

const props = withDefaults(
    defineProps<{
        searchableItemCount: number
        contentTypeLabel: string
        busy?: boolean
        busyTooltip?: string | null
        disableAddContent?: boolean
        disableAddContentTooltip?: string
        refreshing?: boolean
        viewDependencies?: boolean
    }>(),
    {
        busy: false,
        busyTooltip: null,
        disableAddContent: false,
        disableAddContentTooltip: undefined,
        refreshing: false,
        viewDependencies: false,
    },
)

const emit = defineEmits<{
    browse: []
    refresh: []
    viewDependencies: []
}>()
</script>

<template>
    <div class="flex flex-wrap items-center gap-2">
        <StyledInput
            v-model="searchQuery"
            :icon="SearchIcon"
            type="text"
            autocomplete="off"
            :spellcheck="false"
            input-class="!h-10"
            wrapper-class="flex-1 min-w-0"
            clearable
            :placeholder="
                formatMessage(messages.searchPlaceholder, {
                    count: props.searchableItemCount,
                    contentType: formatContentTypeSentence(
                        formatMessage,
                        props.contentTypeLabel,
                        props.searchableItemCount,
                    ),
                })
            "
        />

        <div class="flex items-center gap-2">
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
            <Button
                v-if="props.viewDependencies"
                v-tooltip="formatMessage(messages.viewDependencies)"
                type="outlined"
                :disabled="props.busy"
                class="!h-10 flex items-center gap-2"
                @click="emit('viewDependencies')"
                ><GitGraphIcon class="size-5" />
                <span>{{ formatMessage(messages.viewDependencies) }}</span>
            </Button>
            <Button
                v-tooltip="props.busyTooltip"
                type="outlined"
                :disabled="props.refreshing"
                class="!h-10"
                @click="emit('refresh')"
                ><RefreshCwIcon :class="['size-5', { 'animate-spin': props.refreshing }]" />
                {{ formatMessage(commonMessages.refreshButton) }}
            </Button>
        </div>
        <slot name="actions" />
    </div>
</template>
