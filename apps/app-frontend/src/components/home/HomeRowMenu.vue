<script setup lang="ts">
import './home-widget.css'

import { MoreVerticalIcon } from '@modrinth/assets'
import { defineMessages, type MenuOption, OverflowMenu, useVIntl } from '@modrinth/ui'

/** The "more" button beside a Home row's play button. */
defineProps<{ options: MenuOption[] }>()

const { formatMessage } = useVIntl()
const messages = defineMessages({
    moreOptions: { id: 'app.home.servers.more-options', defaultMessage: 'More options' },
})
</script>

<template>
    <OverflowMenu
        class="home-row-menu"
        :options="options"
        :tooltip="formatMessage(messages.moreOptions)"
    >
        <MoreVerticalIcon aria-hidden="true" />
    </OverflowMenu>
</template>
