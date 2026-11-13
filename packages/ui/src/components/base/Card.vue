<script setup lang="ts">
import { DropdownIcon } from '@modrinth/assets'
import { reactive } from 'vue'

import { defineMessages, useVIntl } from '../../composables/i18n'
import Button from './buttons/Button.vue'

const props = defineProps({
    collapsible: {
        type: Boolean,
        default: false,
    },
    defaultCollapsed: {
        type: Boolean,
        default: false,
    },
    noAutoBody: {
        type: Boolean,
        default: false,
    },
})

const { formatMessage } = useVIntl()
const messages = defineMessages({
    expand: {
        id: 'button.expand',
        defaultMessage: 'Expand',
    },
    collapse: {
        id: 'button.collapse',
        defaultMessage: 'Collapse',
    },
})

const state = reactive({
    collapsed: props.defaultCollapsed,
})

function toggleCollapsed() {
    state.collapsed = !state.collapsed
}
</script>

<template>
    <div class="card">
        <div v-if="!!$slots.header || collapsible" class="header flex">
            <slot name="header"></slot>
            <div v-if="collapsible" class="btn-group ml-auto">
                <Button
                    circular
                    icon-only
                    :aria-expanded="!state.collapsed"
                    :label="formatMessage(state.collapsed ? messages.expand : messages.collapse)"
                    @click="toggleCollapsed"
                    ><DropdownIcon
                        :style="{ transform: `rotate(${state.collapsed ? 0 : 180}deg)` }"
                    />
                </Button>
            </div>
        </div>
        <slot v-if="!state.collapsed" />
    </div>
</template>

<style lang="scss" scoped>
.header {
    :deep(h1, h2, h3, h4) {
        margin-block: 0;
    }

    &:not(:last-child) {
        margin-bottom: var(--gap-lg);
    }
}
</style>
