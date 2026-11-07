<script setup lang="ts">
import { GameIcon, LeftArrowIcon } from '@modrinth/assets'
import { ButtonLink, defineMessages, FormattedTag, useVIntl } from '@modrinth/ui'
import { computed } from 'vue'
import { RouterLink } from 'vue-router'

import InstanceIcon from '@/components/ui/InstanceIcon.vue'

type Instance = {
    id: string
    game_version: string
    loader: string
    path: string
    install_stage: string
    icon_path?: string
    name: string
}

const props = withDefaults(
    defineProps<{
        instance: Instance
        backTab?: string
    }>(),
    { backTab: undefined },
)

const instanceLink = computed(() => {
    const base = `/instance/${encodeURIComponent(props.instance.id)}`
    return props.backTab ? `${base}/${props.backTab}` : base
})
const { formatMessage } = useVIntl()
const messages = defineMessages({
    backToInstance: { id: 'app.instance.back', defaultMessage: 'Back to instance' },
})
</script>

<template>
    <div
        class="flex justify-between items-center border-0 border-b border-solid border-divider pb-4"
    >
        <router-link
            :to="instanceLink"
            tabindex="-1"
            class="flex flex-col gap-4 text-[var(--color-text-default)]"
        >
            <span class="flex items-center gap-2">
                <InstanceIcon
                    :icon-path="instance.icon_path"
                    :instance-id="instance.id"
                    :loader="instance.loader"
                    :alt="instance.name"
                    size="48px"
                />
                <span class="flex flex-col gap-2">
                    <span class="font-extrabold bold text-[var(--color-text-primary)]">
                        {{ instance.name }}
                    </span>
                    <span
                        class="text-[var(--color-text-tertiary)] flex items-center gap-2 font-semibold"
                    >
                        <GameIcon class="h-5 w-5 text-[var(--color-text-tertiary)]" />
                        <FormattedTag :tag="instance.loader" enforce-type="loader" />
                        {{ instance.game_version }}
                    </span>
                </span>
            </span>
        </router-link>
        <ButtonLink :as="RouterLink" :to="instanceLink">
            <LeftArrowIcon /> {{ formatMessage(messages.backToInstance) }}
        </ButtonLink>
    </div>
</template>

<style scoped lang="scss"></style>
