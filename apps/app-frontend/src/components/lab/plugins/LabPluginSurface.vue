<script setup lang="ts">
import type { Component } from 'vue'
import { provide } from 'vue'

import { createLabPluginContext } from '@/lab/plugins/context'
import { LAB_PLUGIN_CONTEXT } from '@/lab/plugins/types'

const props = defineProps<{
    /** Manifest id of the plugin being rendered. */
    pluginId: string
    /** The plugin component to mount. */
    component: Component
    /** Props the launcher passes to the plugin component, from the slot context. */
    componentProps?: Record<string, unknown>
}>()

provide(LAB_PLUGIN_CONTEXT, createLabPluginContext(props.pluginId))
</script>

<template>
    <component :is="component" v-bind="componentProps ?? {}" />
</template>
