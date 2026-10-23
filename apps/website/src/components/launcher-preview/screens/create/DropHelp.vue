<script setup lang="ts">
import BookIcon from '@modrinth/assets/icons/book.svg?component'
import BracesIcon from '@modrinth/assets/icons/braces.svg?component'
import FolderOpenIcon from '@modrinth/assets/icons/folder-open.svg?component'
import GridIcon from '@modrinth/assets/icons/grid.svg?component'
import MapIcon from '@modrinth/assets/icons/map.svg?component'
import PackageOpenIcon from '@modrinth/assets/icons/package-open.svg?component'
import PaletteIcon from '@modrinth/assets/icons/palette.svg?component'
import SparklesIcon from '@modrinth/assets/icons/sparkles.svg?component'
import BigOptionButton from '@modrinth/ui/src/components/base/BigOptionButton.vue'
import type { MessageDescriptor } from '@modrinth/ui/src/composables/i18n.ts'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import type { Component } from 'vue'

import { dropHelpMessages as messages } from './messages'

/** Port of `apps/app-frontend/src/pages/help/DropHelp.vue` (`/help/drop`). */
const { formatMessage } = useVIntl()

interface HelpSection {
    key: string
    icon: Component
    titleMsg: MessageDescriptor
    descMsg: MessageDescriptor
    formatsMsg: MessageDescriptor
}

const sections: HelpSection[] = [
    {
        key: 'instances',
        icon: FolderOpenIcon,
        titleMsg: messages.instancesTitle,
        descMsg: messages.instancesDesc,
        formatsMsg: messages.instancesFormats,
    },
    {
        key: 'mods',
        icon: PackageOpenIcon,
        titleMsg: messages.modsTitle,
        descMsg: messages.modsDesc,
        formatsMsg: messages.modsFormats,
    },
    {
        key: 'modpacks',
        icon: GridIcon,
        titleMsg: messages.modpacksTitle,
        descMsg: messages.modpacksDesc,
        formatsMsg: messages.modpacksFormats,
    },
    {
        key: 'resource-packs',
        icon: PaletteIcon,
        titleMsg: messages.resourcePacksTitle,
        descMsg: messages.resourcePacksDesc,
        formatsMsg: messages.resourcePacksFormats,
    },
    {
        key: 'data-packs',
        icon: BracesIcon,
        titleMsg: messages.dataPacksTitle,
        descMsg: messages.dataPacksDesc,
        formatsMsg: messages.dataPacksFormats,
    },
    {
        key: 'shader-packs',
        icon: SparklesIcon,
        titleMsg: messages.shaderPacksTitle,
        descMsg: messages.shaderPacksDesc,
        formatsMsg: messages.shaderPacksFormats,
    },
    {
        key: 'worlds',
        icon: MapIcon,
        titleMsg: messages.worldsTitle,
        descMsg: messages.worldsDesc,
        formatsMsg: messages.worldsFormats,
    },
    {
        key: 'schematics',
        icon: BookIcon,
        titleMsg: messages.schematicsTitle,
        descMsg: messages.schematicsDesc,
        formatsMsg: messages.schematicsFormats,
    },
]
</script>

<template>
    <div class="flex flex-col gap-3 p-6">
        <h1 class="text-lg font-bold text-[var(--color-text-primary)]">
            {{ formatMessage(messages.title) }}
        </h1>

        <div class="flex flex-col gap-2">
            <BigOptionButton
                v-for="section in sections"
                :key="section.key"
                :icon="section.icon"
                :title="formatMessage(section.titleMsg)"
                :description="formatMessage(section.descMsg)"
                :note="formatMessage(section.formatsMsg)"
                no-icon-border
            />
        </div>
    </div>
</template>
