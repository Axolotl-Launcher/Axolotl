<script setup lang="ts">
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import type { ComboboxOption } from '@modrinth/ui/src/components/base/Combobox.vue'
import Combobox from '@modrinth/ui/src/components/base/Combobox.vue'
import Toggle from '@modrinth/ui/src/components/base/Toggle.vue'
import { defineMessages, useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { computed } from 'vue'

import {
    isServerTypeSupported,
    listServerTypes,
    type ServerTypeDefinition,
    type ServerTypeId,
} from '../../../../lib/server-core.ts'
import { injectCreateServerFlow } from '../create-server-flow.ts'
import { SERVER_TYPE_META } from '../server-type.ts'

const { formatMessage } = useVIntl()
const ctx = injectCreateServerFlow()

const messages = defineMessages({
    heading: { id: 'app.servers.wizard.type-heading', defaultMessage: 'Choose a server core' },
    gameVersion: { id: 'app.servers.wizard.game-version', defaultMessage: 'Game version' },
    loaderVersion: { id: 'app.servers.wizard.loader-version', defaultMessage: 'Loader version' },
    showSnapshots: { id: 'app.servers.wizard.show-snapshots', defaultMessage: 'Show snapshots' },
    loading: { id: 'app.servers.wizard.loading-versions', defaultMessage: 'Loading versions…' },
    noLoaders: {
        id: 'app.servers.wizard.no-loader-versions',
        defaultMessage: 'No loader versions are available for this game version.',
    },
    retry: { id: 'app.servers.wizard.retry', defaultMessage: 'Retry' },
})

const typeLabels = defineMessages({
    vanilla: { id: 'app.servers.type.vanilla', defaultMessage: 'Vanilla' },
    fabric: { id: 'app.servers.type.fabric', defaultMessage: 'Fabric' },
    paper: { id: 'app.servers.type.paper', defaultMessage: 'Paper' },
    forge: { id: 'app.servers.type.forge', defaultMessage: 'Forge' },
    neoforge: { id: 'app.servers.type.neoforge', defaultMessage: 'NeoForge' },
})

const SERVER_TYPE_ORDER: ServerTypeId[] = ['vanilla', 'fabric', 'forge', 'neoforge', 'paper']

function serverTypeLabel(type: ServerTypeDefinition): string {
    const message = typeLabels[type.id as keyof typeof typeLabels]
    return message ? formatMessage(message) : type.label
}

const serverTypeOptions = listServerTypes()
    .filter((type) => isServerTypeSupported(type.id))
    .sort((a, b) => SERVER_TYPE_ORDER.indexOf(a.id) - SERVER_TYPE_ORDER.indexOf(b.id))

const gameVersionOptions = computed<ComboboxOption<string>[]>(() =>
    ctx.availableGameVersions.value.map((version) => ({ value: version, label: version })),
)

const loaderVersionOptions = computed<ComboboxOption<string>[]>(() =>
    ctx.loaderVersions.value.map((loader) => ({ value: loader.id, label: loader.id })),
)

function selectType(typeId: string) {
    ctx.serverType.value = typeId as ServerTypeId
    void ctx.loadLoaderVersions()
}

function selectGameVersion(version: string) {
    ctx.selectedGameVersion.value = version
    void ctx.loadLoaderVersions()
}

// Inline styles instead of Tailwind arbitrary values: underscores inside
// `var(--_color)` are converted to spaces by Tailwind's arbitrary-value
// parsing, which generates invalid CSS and breaks the production build.
const monogramStyles = computed<Record<string, string>>(() =>
    Object.fromEntries(
        serverTypeOptions.map((type) => [
            type.id,
            `color-mix(in srgb, ${SERVER_TYPE_META[type.id].colorVar} 14%, transparent)`,
        ]),
    ),
)
</script>

<template>
    <div class="flex flex-col gap-5">
        <div>
            <h2 class="m-0 text-lg font-semibold text-[var(--color-text-primary)]">
                {{ formatMessage(messages.heading) }}
            </h2>
        </div>

        <div class="grid grid-cols-1 gap-2 sm:grid-cols-3">
            <button
                v-for="type in serverTypeOptions"
                :key="type.id"
                type="button"
                class="flex items-center gap-2.5 rounded-lg border border-solid px-3 py-2.5 text-left transition-colors"
                :class="
                    ctx.serverType.value === type.id
                        ? 'border-brand bg-brand-highlight'
                        : 'border-surface-4 bg-surface-2 hover:border-surface-5'
                "
                @click="selectType(type.id)"
            >
                <span
                    v-if="SERVER_TYPE_META[type.id].icon"
                    class="flex size-7 shrink-0 items-center justify-center overflow-hidden"
                >
                    <img
                        :src="SERVER_TYPE_META[type.id].icon"
                        :alt="serverTypeLabel(type)"
                        class="size-full object-contain"
                    />
                </span>
                <span
                    v-else
                    class="flex size-7 shrink-0 items-center justify-center rounded-md text-xs font-bold"
                    :style="{
                        color: SERVER_TYPE_META[type.id].colorVar,
                        backgroundColor: monogramStyles[type.id],
                    }"
                >
                    {{ SERVER_TYPE_META[type.id].monogram }}
                </span>
                <span class="min-w-0 truncate font-semibold text-[var(--color-text-primary)]">{{
                    serverTypeLabel(type)
                }}</span>
            </button>
        </div>

        <div class="flex items-end justify-between gap-4">
            <div class="flex min-w-0 flex-1 flex-col gap-2">
                <span class="font-semibold text-[var(--color-text-primary)]">
                    {{ formatMessage(messages.gameVersion) }}
                </span>
                <Combobox
                    :model-value="ctx.selectedGameVersion.value"
                    :options="gameVersionOptions"
                    :placeholder="formatMessage(messages.gameVersion)"
                    @update:model-value="selectGameVersion"
                />
            </div>

            <div class="flex shrink-0 items-center gap-2 pb-2.5">
                <span class="whitespace-nowrap text-sm text-[var(--color-text-tertiary)]">
                    {{ formatMessage(messages.showSnapshots) }}
                </span>
                <Toggle
                    id="wizard-show-snapshots"
                    :model-value="ctx.showSnapshots.value"
                    small
                    @update:model-value="
                        (value) => {
                            ctx.showSnapshots.value = !!value
                            void ctx.loadVersions()
                        }
                    "
                />
            </div>
        </div>

        <div v-if="ctx.needsLoaderVersion.value" class="flex min-w-0 flex-col gap-2">
            <span class="font-semibold text-[var(--color-text-primary)]">
                {{ formatMessage(messages.loaderVersion) }}
            </span>
            <Combobox
                :model-value="ctx.selectedLoaderVersion.value"
                :options="loaderVersionOptions"
                :placeholder="formatMessage(messages.loaderVersion)"
                @update:model-value="(value) => (ctx.selectedLoaderVersion.value = value)"
            />
            <p
                v-if="ctx.isVersionsLoading.value"
                role="status"
                class="m-0 text-sm text-[var(--color-text-tertiary)]"
            >
                {{ formatMessage(messages.loading) }}
            </p>
            <p
                v-else-if="!ctx.versionsError.value && !ctx.loaderVersions.value.length"
                role="status"
                class="m-0 text-sm text-[var(--color-text-tertiary)]"
            >
                {{ formatMessage(messages.noLoaders) }}
            </p>
        </div>
        <div
            v-if="ctx.versionsError.value"
            role="alert"
            class="flex items-center justify-between gap-3"
        >
            <p class="m-0 text-sm text-[var(--color-text-tertiary)]">
                {{ ctx.versionsError.value }}
            </p>
            <Button type="transparent" size="sm" @click="ctx.loadVersions()">{{
                formatMessage(messages.retry)
            }}</Button>
        </div>
    </div>
</template>
