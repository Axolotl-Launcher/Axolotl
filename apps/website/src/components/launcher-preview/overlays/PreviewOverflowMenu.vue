<script setup lang="ts">
import OverflowMenu from '@modrinth/ui/src/components/base/OverflowMenu.vue'

import { injectPreviewWindow } from '../lib/window'

/** `@modrinth/ui` OverflowMenu, opened inside the launcher window. */
defineOptions({ inheritAttrs: false })
const { layer } = injectPreviewWindow()
</script>

<template>
    <OverflowMenu v-bind="$attrs" :container="layer ?? undefined">
        <template v-for="(_, name) in $slots" #[name]="scope"
            ><slot :name="name" v-bind="scope ?? {}"
        /></template>
    </OverflowMenu>
</template>
