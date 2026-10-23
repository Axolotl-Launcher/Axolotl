<script setup lang="ts">
import Avatar from '@modrinth/ui/src/components/base/Avatar.vue'
import { computed } from 'vue'

import { getDisplayInstanceIcon } from '../lib/instance-icons'
import { loaderNames } from '../lib/state/instances'
import type { InstanceLoader } from '../lib/types'

const props = withDefaults(
    defineProps<{
        iconPath?: string | null
        instanceId?: string | null
        loader?: string | null
        size?: string
    }>(),
    { iconPath: null, instanceId: null, loader: null, size: '32px' },
)

/** Port of the app's InstanceIcon: the instance image, or the loader's built-in icon. */
const display = computed(() => getDisplayInstanceIcon(props.iconPath, props.loader))

const label = computed(() =>
    props.loader ? (loaderNames[props.loader as InstanceLoader] ?? 'Minecraft') : 'Minecraft',
)
</script>

<template>
    <Avatar
        :src="display.url"
        :alt="label"
        :size="size"
        :tint-by="instanceId"
        :class="{ '!rounded-none !border-0 !bg-transparent !shadow-none': display.frameless }"
    />
</template>
