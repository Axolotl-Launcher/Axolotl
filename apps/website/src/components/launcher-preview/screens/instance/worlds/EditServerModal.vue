<script setup lang="ts">
import SaveIcon from '@modrinth/assets/icons/save.svg?component'
import XIcon from '@modrinth/assets/icons/x.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import { defineMessage, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { injectNotificationManager } from '@modrinth/ui/src/providers/web-notifications.ts'
import { commonMessages } from '@modrinth/ui/src/utils/common-messages.ts'
import { ref } from 'vue'

import type { GameInstance } from '../../../lib/types.ts'
import NewModal from '../../../overlays/PreviewModal.vue'
import SymlinkInstanceWarning from '../components/SymlinkInstanceWarning.vue'
import ServerModalBody from './ServerModalBody.vue'
import {
    type DisplayStatus,
    edit_server_in_instance,
    type ServerPackStatus,
    type ServerWorld,
} from './worlds-api.ts'

const { handleError } = injectNotificationManager()
const { formatMessage } = useVIntl()

const emit = defineEmits<{
    submit: [server: ServerWorld]
}>()

const props = defineProps<{
    instance: GameInstance
}>()

const modal = ref<InstanceType<typeof NewModal>>()

const name = ref<string>('')
const address = ref<string>('')
const resourcePack = ref<ServerPackStatus>('enabled')
const index = ref<number>(0)
const displayStatus = ref<DisplayStatus>('normal')
async function saveServer() {
    const serverName = name.value ? name.value : address.value
    const resourcePackStatus = resourcePack.value
    await edit_server_in_instance(
        props.instance.id,
        index.value,
        serverName,
        address.value,
        resourcePackStatus,
    ).catch(handleError)

    emit('submit', {
        name: serverName,
        type: 'server',
        index: index.value,
        address: address.value,
        pack_status: resourcePackStatus,
        display_status: displayStatus.value,
    })
    hide()
}

function show(server: ServerWorld) {
    name.value = server.name
    address.value = server.address
    resourcePack.value = server.pack_status
    index.value = server.index
    displayStatus.value = server.display_status
    modal.value?.show()
}

function hide() {
    modal.value?.hide()
}

defineExpose({ show })

const titleMessage = defineMessage({
    id: 'instance.edit-server.title',
    defaultMessage: 'Edit server',
})
</script>
<template>
    <NewModal ref="modal" :header="formatMessage(titleMessage)" width="500px" max-width="500px">
        <SymlinkInstanceWarning
            v-if="props.instance?.symlink_target"
            :symlink-target="props.instance.symlink_target"
        />
        <ServerModalBody
            v-model:name="name"
            v-model:address="address"
            v-model:resource-pack="resourcePack"
        />
        <template #actions>
            <div class="flex justify-end gap-2">
                <Button type="outlined" @click="hide()"
                    ><XIcon />
                    {{ formatMessage(commonMessages.cancelButton) }}
                </Button>
                <Button type="colored" color="brand" :disabled="!address" @click="saveServer"
                    ><SaveIcon />
                    {{ formatMessage(commonMessages.saveChangesButton) }}
                </Button>
            </div>
        </template>
    </NewModal>
</template>
