<script setup lang="ts">
import Avatar from '@modrinth/ui/src/components/base/Avatar.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { ref } from 'vue'

import PreviewModal from '../../../../../overlays/PreviewModal.vue'
import { messages } from '../../../messages'
import cyf112233Avatar from './cyf112233.jpg'

/**
 * The contributor easter egg the Konami code opens on the About page
 * (apps/app-frontend/src/components/ui/easteregg/EasterEggContributorsModal.vue).
 */
const emit = defineEmits<{
    openGame: []
}>()

const { formatMessage } = useVIntl()

const modal = ref<InstanceType<typeof PreviewModal> | null>(null)

const contributors = [
    {
        name: 'cyf112233',
        avatarUrl: cyf112233Avatar,
    },
]

function show() {
    modal.value?.show()
}

function selectContributor() {
    modal.value?.hide()
    emit('openGame')
}

defineExpose({ show })
</script>

<template>
    <PreviewModal
        ref="modal"
        :header="formatMessage(messages.appSettingsAboutEastereggContributorsTitle)"
        width="min(480px, calc(1280px - 2rem))"
        max-width="480px"
    >
        <ul class="m-0 list-none p-0">
            <li>
                <button
                    type="button"
                    class="flex w-full items-center gap-3 rounded-xl bg-surface-4 p-4 text-left transition-colors hover:bg-surface-5"
                    @click="selectContributor"
                >
                    <Avatar
                        :src="contributors[0].avatarUrl"
                        :alt="contributors[0].name"
                        size="4rem"
                        circle
                        no-shadow
                    />
                    <span class="min-w-0">
                        <span class="block font-semibold text-[var(--color-text-primary)]">{{
                            contributors[0].name
                        }}</span>
                        <span class="block text-sm text-[var(--color-text-tertiary)]">
                            {{ formatMessage(messages.appSettingsAboutEastereggClickHint) }}
                        </span>
                    </span>
                </button>
            </li>
        </ul>
    </PreviewModal>
</template>
