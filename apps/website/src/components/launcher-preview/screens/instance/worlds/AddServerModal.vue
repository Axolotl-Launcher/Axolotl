<script setup lang="ts">
import PlayIcon from '@modrinth/assets/icons/play.svg?component'
import PlusIcon from '@modrinth/assets/icons/plus.svg?component'
import XIcon from '@modrinth/assets/icons/x.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { injectNotificationManager } from '@modrinth/ui/src/providers/web-notifications.ts'
import { commonMessages } from '@modrinth/ui/src/utils/common-messages.ts'
import { ref } from 'vue'

import type { GameInstance } from '../../../lib/types.ts'
import NewModal from '../../../overlays/PreviewModal.vue'
import SymlinkInstanceWarning from '../components/SymlinkInstanceWarning.vue'
import ServerModalBody from './ServerModalBody.vue'
import { add_server_to_instance, type ServerPackStatus, type ServerWorld } from './worlds-api.ts'

const { handleError } = injectNotificationManager()
const { formatMessage } = useVIntl()

const emit = defineEmits<{
    submit: [server: ServerWorld, play: boolean]
}>()

const props = defineProps<{
    instance: GameInstance
}>()

const modal = ref<InstanceType<typeof NewModal>>()

const name = ref('')
const address = ref('')
const resourcePack = ref<ServerPackStatus>('enabled')

async function addServer(play: boolean) {
    const serverName = name.value ? name.value : address.value
    const resourcePackStatus = resourcePack.value
    const index =
        (await add_server_to_instance(
            props.instance.id,
            serverName,
            address.value,
            resourcePackStatus,
        ).catch(handleError)) ?? 0
    emit(
        'submit',
        {
            name: serverName,
            type: 'server',
            index,
            address: address.value,
            pack_status: resourcePackStatus,
        },
        play,
    )
    hide()
}

function show() {
    name.value = ''
    address.value = ''
    resourcePack.value = 'enabled'
    modal.value?.show()
}

function hide() {
    modal.value?.hide()
}

const messages = defineMessages({
    title: {
        id: 'instance.add-server.title',
        defaultMessage: 'Add a server',
    },
    addServer: {
        id: 'instance.add-server.add-server',
        defaultMessage: 'Add server',
    },
    addAndPlay: {
        id: 'instance.add-server.add-and-play',
        defaultMessage: 'Add and play',
    },
})

defineExpose({ show, hide })
</script>
<template>
    <NewModal ref="modal" :header="formatMessage(messages.title)" width="500px" max-width="500px">
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
                <Button :disabled="!address" @click="addServer(false)"
                    ><PlusIcon />
                    {{ formatMessage(messages.addServer) }}
                </Button>
                <Button type="colored" color="brand" :disabled="!address" @click="addServer(true)"
                    ><PlayIcon />
                    {{ formatMessage(messages.addAndPlay) }}
                </Button>
            </div>
        </template>
    </NewModal>
</template>
