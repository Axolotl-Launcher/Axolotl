<script setup lang="ts">
import Avatar from '@modrinth/ui/src/components/base/Avatar.vue'
import { computed } from 'vue'

import { getDisplayInstanceIcon } from '../../../lib/instance-icons'

/**
 * Port of the app's `components/ui/InstanceIcon.vue`: the instance's own icon,
 * or the built-in icon of its loader. Size and class fall through to the
 * `Avatar`, as in the app.
 */
const props = withDefaults(
    defineProps<{
        iconPath?: string | null
        instanceId?: string | null
        loader?: string | null
    }>(),
    {
        iconPath: null,
        instanceId: null,
        loader: null,
    },
)

const displayIcon = computed(() => getDisplayInstanceIcon(props.iconPath, props.loader))
</script>

<template>
    <Avatar
        :src="displayIcon.url"
        :tint-by="instanceId"
        :class="{ '!rounded-none !border-0 !bg-transparent !shadow-none': displayIcon.frameless }"
    />
</template>
