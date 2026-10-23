<script setup lang="ts">
import Avatar from '@modrinth/ui/src/components/base/Avatar.vue'
import { computed } from 'vue'

import { builtInInstanceIcons, isBuiltInInstanceIcon } from '../../../lib/instance-icons'
import type { LocalServer } from '../../../lib/types'

/**
 * Port of `components/multiplayer/servers/ServerIcon.vue`: a user-set icon
 * wins, otherwise the server type's brand icon, otherwise a coloured monogram.
 * Brand icons render frameless.
 */
const props = withDefaults(
    defineProps<{
        iconPath?: string | null
        serverType: LocalServer['server_type']
        serverId?: string | null
        size?: string
    }>(),
    {
        iconPath: null,
        serverId: null,
        size: '2rem',
    },
)

interface ServerTypeMeta {
    colorVar: string
    monogram: string
    icon?: string
}

const PLATFORM = (id: string) => `var(--color-platform-${id})`

/** The app's `instance-icons/Fabric.png`, which is also the built-in Fabric icon. */
const FABRIC_ICON = builtInInstanceIcons.find((icon) => icon.id === 'fabric')?.url

/**
 * `SERVER_TYPE_META` (components/multiplayer/servers/server-type.ts). The
 * preview ships the Fabric and Forge artwork; the Mojang and Paper logos are
 * not among its assets, so those types show their monograms.
 */
const SERVER_TYPE_META: Record<LocalServer['server_type'], ServerTypeMeta> = {
    vanilla: { colorVar: 'var(--color-brand)', monogram: 'V' },
    fabric: {
        colorVar: PLATFORM('fabric'),
        monogram: 'F',
        icon: FABRIC_ICON,
    },
    paper: { colorVar: PLATFORM('paper'), monogram: 'P' },
    forge: {
        colorVar: PLATFORM('forge'),
        monogram: 'Fo',
        icon: '/launcher-preview/loaders/forge.webp',
    },
    neoforge: { colorVar: PLATFORM('neoforge'), monogram: 'N' },
    quilt: { colorVar: PLATFORM('quilt'), monogram: 'Q' },
    purpur: { colorVar: PLATFORM('purpur'), monogram: 'Pu' },
}

const typeMeta = computed(() => SERVER_TYPE_META[props.serverType] ?? SERVER_TYPE_META.vanilla)
const displayUrl = computed(() => props.iconPath ?? typeMeta.value.icon ?? null)
const frameless = computed(() => {
    if (props.iconPath) return isBuiltInInstanceIcon(props.iconPath)
    return !!typeMeta.value.icon
})

// Inline styles instead of Tailwind arbitrary values, as in the app.
const monogramStyle = computed(() => ({
    color: typeMeta.value.colorVar,
    backgroundColor: `color-mix(in srgb, ${typeMeta.value.colorVar} 14%, transparent)`,
}))
</script>

<template>
    <Avatar
        v-if="displayUrl"
        :src="displayUrl"
        :size="size"
        :tint-by="serverId"
        :class="{ '!rounded-none !border-0 !bg-transparent !shadow-none': frameless }"
    />
    <div
        v-else
        class="flex shrink-0 items-center justify-center rounded-lg text-xs font-bold"
        :style="{
            '--_size': size,
            width: 'var(--_size)',
            height: 'var(--_size)',
            fontSize: 'calc(var(--_size) * 0.375)',
            ...monogramStyle,
        }"
    >
        {{ typeMeta.monogram }}
    </div>
</template>
