<template>
    <Card>
        <ResourceDescription source="modrinth" :project-id="project.id">
            <TranslatedProjectDescription
                :description="project.body"
                :active="translationActive"
                :translations="translations"
                :mode="translationMode"
                :style="translationStyle"
            />
        </ResourceDescription>
    </Card>
</template>

<script setup>
import { Card } from '@modrinth/ui'

import ResourceDescription from '@/components/ui/ResourceDescription.vue'
import TranslatedProjectDescription from '@/components/ui/TranslatedProjectDescription.vue'

defineProps({
    project: {
        type: Object,
        default: () => {},
    },
    translationActive: {
        type: Boolean,
        default: false,
    },
    translations: {
        type: Object,
        default: () => ({}),
    },
    translationMode: {
        type: String,
        default: 'bilingual',
    },
    translationStyle: {
        type: String,
        default: 'weakened',
    },
})
</script>

<script>
export default {
    name: 'Description',
}
</script>
