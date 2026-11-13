<script setup lang="ts">
import { Button, defineMessages, NewModal, useVIntl } from '@modrinth/ui'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { computed, ref } from 'vue'

import {
    isSyncedPackTypeAvailable,
    list_synced_packs,
    remove_synced_pack,
    set_synced_pack_enabled,
    syncedPackKeys,
    type SyncedPackType,
} from '@/helpers/synced-packs'

const modal = ref<InstanceType<typeof NewModal>>()
const type = ref<SyncedPackType>('resourcepack')
const queryClient = useQueryClient()
const { formatMessage } = useVIntl()
const messages = defineMessages({
    resourcePacks: {
        id: 'app.settings.synced-packs.resource-packs',
        defaultMessage: 'Resource packs',
    },
    dataPacks: { id: 'app.settings.synced-packs.data-packs', defaultMessage: 'Data packs' },
    enable: { id: 'app.settings.synced-packs.enable', defaultMessage: 'Enable' },
    disable: { id: 'app.settings.synced-packs.disable', defaultMessage: 'Disable' },
    remove: { id: 'app.settings.synced-packs.remove', defaultMessage: 'Remove' },
    empty: { id: 'app.settings.synced-packs.empty', defaultMessage: 'No synced packs' },
})
const query = useQuery({
    queryKey: computed(() => syncedPackKeys.list(type.value)),
    queryFn: () => list_synced_packs(type.value),
    enabled: computed(() => isSyncedPackTypeAvailable(type.value)),
})
const packs = computed(() => query.data.value ?? [])
const mutation = useMutation({
    mutationFn: async (request: { id: string; enabled?: boolean; remove?: boolean }) => {
        if (request.remove) await remove_synced_pack(request.id)
        else await set_synced_pack_enabled(request.id, request.enabled ?? true)
    },
    onSuccess: () => queryClient.invalidateQueries({ queryKey: syncedPackKeys.all }),
})

async function show(nextType: SyncedPackType) {
    type.value = nextType
    modal.value?.show()
    await queryClient.invalidateQueries({ queryKey: syncedPackKeys.list(nextType) })
}

function hide() {
    modal.value?.hide()
}

defineExpose({ show, hide })
</script>

<template>
    <NewModal
        ref="modal"
        :header="
            formatMessage(type === 'resourcepack' ? messages.resourcePacks : messages.dataPacks)
        "
        max-width="42rem"
    >
        <div v-if="packs.length" class="flex flex-col gap-2">
            <div
                v-for="pack in packs"
                :key="pack.id"
                class="flex items-center gap-3 border-b border-solid border-surface-4 py-2"
            >
                <span class="min-w-0 flex-1 truncate">{{ pack.file_name }}</span>
                <Button
                    :disabled="mutation.isPending.value"
                    @click="mutation.mutate({ id: pack.id, enabled: !pack.enabled })"
                    >{{ formatMessage(pack.enabled ? messages.disable : messages.enable) }}
                </Button>
                <Button
                    type="colored"
                    color="red"
                    :disabled="mutation.isPending.value"
                    @click="mutation.mutate({ id: pack.id, remove: true })"
                    >{{ formatMessage(messages.remove) }}
                </Button>
            </div>
        </div>
        <p v-else class="m-0 text-[var(--color-text-tertiary)]">
            {{ formatMessage(messages.empty) }}
        </p>
    </NewModal>
</template>
