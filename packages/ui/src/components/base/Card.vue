<script setup lang="ts">
import { DropdownIcon } from '@modrinth/assets'
import { computed, inject, reactive } from 'vue'

import { I18N_INJECTION_KEY } from '../../providers/i18n'
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

function label(id: string, fallback: string) {
    const translated = i18n?.t(id)
    return !translated || translated === id ? fallback : translated
}

const expandLabel = computed(() => label('button.expand', 'Expand'))
const collapseLabel = computed(() => label('button.collapse', 'Collapse'))

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
