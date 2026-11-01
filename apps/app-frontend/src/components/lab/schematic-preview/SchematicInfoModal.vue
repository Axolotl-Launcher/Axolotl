<script setup lang="ts">
import { ScanEyeIcon, TriangleAlertIcon } from '@modrinth/assets'
import { Button, Checkbox, defineMessages, NewModal, useVIntl } from '@modrinth/ui'
import { useTemplateRef } from 'vue'

import type { SchematicPreviewManifest, SchematicRegion } from '@/lab/schematic-preview/backend'

defineProps<{
    manifest: SchematicPreviewManifest
    format: string
    warnings: string[]
    regionVisibility: Record<string, boolean>
}>()

const emit = defineEmits<{
    regionVisibility: [regionId: string, visible: boolean]
    focusRegion: [region: SchematicRegion]
}>()

const { formatMessage, formatNumber } = useVIntl()
const modal = useTemplateRef<InstanceType<typeof NewModal>>('modal')

const messages = defineMessages({
    title: { id: 'app.lab.schematic-preview.info.title', defaultMessage: 'Schematic info' },
    metadata: { id: 'app.lab.schematic-preview.metadata', defaultMessage: 'Metadata' },
    regions: { id: 'app.lab.schematic-preview.regions', defaultMessage: 'Regions' },
    focusRegion: { id: 'app.lab.schematic-preview.focus-region', defaultMessage: 'Focus region' },
    author: { id: 'app.lab.schematic-preview.author', defaultMessage: 'Author' },
    format: { id: 'app.lab.schematic-preview.format', defaultMessage: 'Format' },
    dataVersion: { id: 'app.lab.schematic-preview.data-version', defaultMessage: 'Data version' },
    coordinates: { id: 'app.lab.schematic-preview.coordinates', defaultMessage: 'Coordinates' },
    blocks: { id: 'app.lab.schematic-preview.blocks', defaultMessage: 'Blocks' },
    entities: { id: 'app.lab.schematic-preview.entities', defaultMessage: 'Entities' },
    blockEntities: {
        id: 'app.lab.schematic-preview.block-entities',
        defaultMessage: 'Block entities',
    },
    warnings: { id: 'app.lab.schematic-preview.warnings', defaultMessage: 'Warnings' },
})

defineExpose({ show: () => modal.value?.show() })
</script>

<template>
    <NewModal
        ref="modal"
        :header="formatMessage(messages.title)"
        width="min(620px, calc(100vw - 2rem))"
        max-width="620px"
        scrollable
        max-content-height="min(42rem, 76vh)"
    >
        <div class="flex min-w-0 flex-col gap-6">
            <section class="info-section">
                <h2>{{ formatMessage(messages.metadata) }}</h2>
                <dl class="metadata-grid">
                    <dt>{{ formatMessage(messages.format) }}</dt>
                    <dd>{{ format }}</dd>
                    <template v-if="manifest.author">
                        <dt>{{ formatMessage(messages.author) }}</dt>
                        <dd>{{ manifest.author }}</dd>
                    </template>
                    <template v-if="manifest.dataVersion">
                        <dt>{{ formatMessage(messages.dataVersion) }}</dt>
                        <dd>{{ manifest.dataVersion }}</dd>
                    </template>
                    <dt>{{ formatMessage(messages.coordinates) }}</dt>
                    <dd>{{ manifest.min.join(', ') }} -> {{ manifest.max.join(', ') }}</dd>
                    <dt>{{ formatMessage(messages.blocks) }}</dt>
                    <dd>{{ formatNumber(manifest.blockCount) }}</dd>
                    <dt>{{ formatMessage(messages.entities) }}</dt>
                    <dd>{{ formatNumber(manifest.entityCount) }}</dd>
                    <dt>{{ formatMessage(messages.blockEntities) }}</dt>
                    <dd>{{ formatNumber(manifest.blockEntityCount) }}</dd>
                </dl>
            </section>

            <section class="info-section">
                <h2>{{ formatMessage(messages.regions) }}</h2>
                <div class="flex flex-col gap-1">
                    <div v-for="region in manifest.regions" :key="region.id" class="region-row">
                        <Checkbox
                            :model-value="regionVisibility[region.id]"
                            :label="region.name"
                            @update:model-value="emit('regionVisibility', region.id, $event)"
                        />
                        <span class="text-xs text-[var(--color-text-tertiary)]">
                            {{ region.size.join(' x ') }} - {{ formatNumber(region.blockCount) }}
                        </span>
                        <Button
                            v-tooltip="formatMessage(messages.focusRegion)"
                            type="quiet"
                            size="2xs"
                            circular
                            icon-only
                            :aria-label="formatMessage(messages.focusRegion)"
                            @click="emit('focusRegion', region)"
                            ><ScanEyeIcon />
                        </Button>
                    </div>
                </div>
            </section>

            <section
                v-if="warnings.length"
                class="info-section flex min-w-0 flex-col gap-3 warning-section text-orange"
            >
                <h2><TriangleAlertIcon />{{ formatMessage(messages.warnings) }}</h2>
                <ul>
                    <li v-for="warning in warnings" :key="warning">{{ warning }}</li>
                </ul>
            </section>
        </div>
    </NewModal>
</template>

<style scoped>
.info-section h2 {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    margin: 0;
    color: var(--color-text-primary);
    font-size: 0.9rem;
}

.info-section h2 svg {
    width: 1rem;
    height: 1rem;
}

.metadata-grid {
    display: grid;
    grid-template-columns: max-content minmax(0, 1fr);
    gap: 0.5rem 1rem;
    margin: 0;
    font-size: 0.8rem;
}

.metadata-grid dt {
    color: var(--color-text-tertiary);
}

.metadata-grid dd {
    min-width: 0;
    margin: 0;
    overflow-wrap: anywhere;
    color: var(--color-text-primary);
    text-align: right;
}

.region-row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto auto;
    align-items: center;
    gap: 0.5rem;
    border-radius: var(--radius-sm);
    padding: 0.5rem;
    background: var(--surface-2);
}

.region-row :deep(.checkbox-outer) {
    min-width: 0;
}

.region-row :deep(.checkbox-outer > span:last-child) {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.warning-section ul {
    display: flex;
    margin: 0;
    flex-direction: column;
    gap: 0.4rem;
    padding-left: 1.15rem;
    font-size: 0.75rem;
}

@media (max-width: 520px) {
    .region-row {
        grid-template-columns: minmax(0, 1fr) auto;
    }

    .region-row > span {
        grid-column: 1 / -1;
        grid-row: 2;
    }
}
</style>
