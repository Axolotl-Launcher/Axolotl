<script setup lang="ts">
import Admonition from '@modrinth/ui/src/components/base/Admonition.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { onMounted, ref } from 'vue'

import DesktopAppLink from '../../../shell/DesktopAppLink.vue'
import { type SeedMapPreviewLimitReason, seedMapPreviewMeta } from './preview-data.ts'
import { seedMapPreviewMessages as messages } from './preview-messages.ts'

/** Shown where the desktop app would compute what the preview's data does not include. */
const props = defineProps<{ reason: SeedMapPreviewLimitReason }>()

const { formatMessage } = useVIntl()
const world = ref({ seed: '', version: '' })

onMounted(async () => {
    if (props.reason !== 'world') return
    const meta = await seedMapPreviewMeta().catch(() => null)
    if (meta) world.value = { seed: meta.seed, version: meta.version }
})
</script>

<template>
    <Admonition
        type="info"
        :header="formatMessage(messages.title)"
        :body="formatMessage(messages[reason], world)"
    >
        <template #actions><DesktopAppLink /></template>
    </Admonition>
</template>
