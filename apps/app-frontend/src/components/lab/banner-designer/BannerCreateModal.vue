<script setup lang="ts">
import { PlusIcon } from '@modrinth/assets'
import { Button, defineMessages, NewModal, useVIntl } from '@modrinth/ui'
import { ref, useTemplateRef } from 'vue'

import BannerColorGrid from '@/components/lab/banner-designer/BannerColorGrid.vue'
import BannerPresetGallery from '@/components/lab/banner-designer/BannerPresetGallery.vue'
import type { DyeColor } from '@/lab/banner-designer/colors.ts'
import type { BannerDesign } from '@/lab/banner-designer/types.ts'

const emit = defineEmits<{
    create: [design: BannerDesign]
}>()

const { formatMessage } = useVIntl()
const modal = useTemplateRef<InstanceType<typeof NewModal>>('modal')

const messages = defineMessages({
    title: { id: 'app.lab.banner-designer.create.title', defaultMessage: 'Create a banner' },
    description: {
        id: 'app.lab.banner-designer.create.description',
        defaultMessage:
            'Choose a base color, or start from a design documented on the Minecraft Wiki.',
    },
    baseColor: {
        id: 'app.lab.banner-designer.create.base-color',
        defaultMessage: 'Base color',
    },
    presets: { id: 'app.lab.banner-designer.create.presets', defaultMessage: 'Presets' },
    cancel: { id: 'app.lab.banner-designer.create.cancel', defaultMessage: 'Cancel' },
    confirm: { id: 'app.lab.banner-designer.create.confirm', defaultMessage: 'Create banner' },
})

const baseColor = ref<DyeColor>('white')

function show() {
    baseColor.value = 'white'
    modal.value?.show()
}

function createBlank() {
    emit('create', { baseColor: baseColor.value, layers: [] })
    modal.value?.hide()
}

function createFromPreset(design: BannerDesign) {
    emit('create', design)
    modal.value?.hide()
}

defineExpose({ show })
</script>

<template>
    <NewModal
        ref="modal"
        :header="formatMessage(messages.title)"
        width="min(46rem, calc(95vw - 10rem))"
        max-width="46rem"
        scrollable
        max-content-height="min(42rem, 82vh)"
    >
        <div class="flex flex-col gap-5">
            <p class="m-0 text-sm text-[var(--color-text-tertiary)]">
                {{ formatMessage(messages.description) }}
            </p>

            <section class="flex flex-col gap-2">
                <h3
                    class="m-0 text-xs font-semibold uppercase tracking-wide text-[var(--color-text-tertiary)]"
                >
                    {{ formatMessage(messages.baseColor) }}
                </h3>
                <BannerColorGrid
                    v-model="baseColor"
                    item-kind="banner"
                    :label="formatMessage(messages.baseColor)"
                />
            </section>

            <section class="flex flex-col gap-2">
                <h3
                    class="m-0 text-xs font-semibold uppercase tracking-wide text-[var(--color-text-tertiary)]"
                >
                    {{ formatMessage(messages.presets) }}
                </h3>
                <BannerPresetGallery @select="createFromPreset" />
            </section>
        </div>

        <template #actions>
            <div class="flex items-center justify-end gap-2">
                <Button type="quiet" @click="modal?.hide()">{{
                    formatMessage(messages.cancel)
                }}</Button>
                <Button type="colored" color="brand" @click="createBlank"
                    ><PlusIcon />{{ formatMessage(messages.confirm) }}
                </Button>
            </div>
        </template>
    </NewModal>
</template>
