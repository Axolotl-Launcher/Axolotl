<script setup lang="ts">
import { DropdownIcon } from '@modrinth/assets'
import { computed, inject, reactive } from 'vue'

import type { MessageDescriptor } from '../../composables/i18n'
import { I18N_INJECTION_KEY } from '../../providers/i18n'
import { commonMessages } from '../../utils/common-messages'
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

const i18n = inject(I18N_INJECTION_KEY, null)

function label(message: MessageDescriptor) {
    // Reading the locale keeps the computed label reactive to language changes.
    void i18n?.locale.value
    const translated = i18n?.t(message.id)
    return !translated || translated === message.id
        ? (message.defaultMessage ?? message.id)
        : translated
}

const expandLabel = computed(() => label(commonMessages.expandButton))
const collapseLabel = computed(() => label(commonMessages.collapseButton))

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
                    :label="state.collapsed ? expandLabel : collapseLabel"
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
