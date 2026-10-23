<script setup lang="ts">
import TrashIcon from '@modrinth/assets/icons/trash.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import { useFormatBytes } from '@modrinth/ui/src/composables/format-bytes.ts'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { ref } from 'vue'

import PreviewModal from '../../../../overlays/PreviewModal.vue'
import { useSettingsPage } from '../../state'
import { useStorageBackend } from './storageBackend'
import type { CacheCleanupResult } from './storageData'
import { storageMessages } from './storageMessages'

/**
 * components/ui/settings/storage/CacheCleanupButton.vue: "Clear cache" on the
 * cache row, its confirmation and the result of `storage_cache_clear`.
 */
const props = withDefaults(defineProps<{ compact?: boolean }>(), { compact: false })
const emit = defineEmits<{ cleared: [] }>()
const { formatMessage } = useVIntl()
const formatBytes = useFormatBytes()
const { getFeatureFlag } = useSettingsPage()
const { purgeLauncherCache } = useStorageBackend()
const modal = ref<InstanceType<typeof PreviewModal> | null>(null)
const busy = ref(false)
const failed = ref(false)
const errorDetail = ref('')
const outcome = ref<CacheCleanupResult | null>(null)

async function show(event: MouseEvent) {
    if (busy.value) return
    failed.value = false
    errorDetail.value = ''
    outcome.value = null
    if (getFeatureFlag('skip_non_essential_warnings')) {
        await clear()
    }
    modal.value?.show(event)
}
async function clear() {
    if (busy.value) return
    busy.value = true
    failed.value = false
    try {
        outcome.value = await purgeLauncherCache()
        emit('cleared')
    } catch (error) {
        failed.value = true
        errorDetail.value = error instanceof Error ? error.message : String(error)
        console.warn('[storage] Cache cleanup failed', error)
    } finally {
        busy.value = false
    }
}
</script>

<template>
    <div class="flex items-center gap-3" @click.stop @keydown.stop>
        <Button :size="props.compact ? '2xs' : 'md'" :loading="busy" @click="show"
            ><TrashIcon />{{ formatMessage(storageMessages.clearCache) }}</Button
        >
    </div>
    <PreviewModal
        ref="modal"
        :header="formatMessage(storageMessages.clearCache)"
        :disable-close="busy"
        width="min(36rem, 1152px)"
    >
        <template v-if="outcome">
            <p role="status" class="m-0">
                {{
                    formatMessage(storageMessages.cacheCleanupResult, {
                        freed: formatBytes(outcome.removed_bytes),
                        remaining: formatBytes(outcome.remaining_bytes),
                    })
                }}
            </p>
            <p v-if="outcome.remaining_bytes" class="text-sm text-[var(--color-text-tertiary)]">
                {{ formatMessage(storageMessages.cacheRetainedHelp) }}
            </p>
            <p v-if="outcome.failures.length" role="alert" class="text-sm text-red">
                {{ formatMessage(storageMessages.cachePartialFailure) }}
            </p>
            <details v-if="outcome.failures.length" class="text-sm">
                <summary>{{ formatMessage(storageMessages.cacheFailureDetails) }}</summary>
                <ul>
                    <li v-for="failure in outcome.failures" :key="failure" class="break-all">
                        {{ failure }}
                    </li>
                </ul>
            </details>
        </template>
        <p v-else class="m-0">{{ formatMessage(storageMessages.clearCacheHelp) }}</p>
        <p v-if="failed" role="alert" class="text-sm text-red">
            {{ formatMessage(storageMessages.cacheClearFailed) }}
        </p>
        <details v-if="failed && errorDetail" class="text-sm">
            <summary>{{ formatMessage(storageMessages.cacheFailureDetails) }}</summary>
            <p class="break-all">{{ errorDetail }}</p>
        </details>
        <template #actions>
            <div class="flex flex-wrap justify-end gap-3">
                <Button :disabled="busy" @click="modal?.hide()">{{
                    formatMessage(outcome ? storageMessages.close : storageMessages.cancel)
                }}</Button>
                <Button v-if="!outcome" type="colored" color="red" :loading="busy" @click="clear">{{
                    formatMessage(busy ? storageMessages.clearingCache : storageMessages.clearCache)
                }}</Button>
            </div>
        </template>
    </PreviewModal>
</template>
