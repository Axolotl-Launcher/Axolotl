<script setup lang="ts">
import { SpinnerIcon } from '@modrinth/assets'
import { Button, useFormatBytes } from '@modrinth/ui'
import { ref } from 'vue'

import {
    type InstalledThemePack,
    type ThemePackCapability,
    themePackDescription,
    themePackDisplayName,
    type ThemePackPermission,
} from '@/helpers/theme-packs'

import { injectThemeSuiteContext } from './context'
import { themeSuiteMessages } from './messages'

const {
    settings,
    packs,
    loading,
    error,
    activeSource,
    selectingSource,
    removingId,
    snapshotExporting,
    snapshotError,
    snapshotExported,
    formatMessage,
    selectSource,
    remove,
    openImport,
    openColors,
    exportSnapshot,
} = injectThemeSuiteContext()

const formatBytes = useFormatBytes()
const confirmingRemove = ref<string | null>(null)
const removeData = ref(false)

function packLocale() {
    return settings.value?.locale ?? 'en-US'
}

function packName(pack: InstalledThemePack) {
    return themePackDisplayName(pack.manifest, packLocale())
}

function packDescription(pack: InstalledThemePack) {
    return themePackDescription(pack.manifest, packLocale())
}

function capabilityLabel(capability: ThemePackCapability) {
    if (capability === 'tokens') return formatMessage(themeSuiteMessages.capabilityTokens)
    if (capability === 'styles') return formatMessage(themeSuiteMessages.capabilityStyles)
    return formatMessage(themeSuiteMessages.capabilityPages)
}

function permissionLabel(permission: ThemePackPermission) {
    if (permission === 'storage') return formatMessage(themeSuiteMessages.permissionStorage)
    if (permission === 'instances.read') {
        return formatMessage(themeSuiteMessages.permissionInstancesRead)
    }
    if (permission === 'instances.logs') {
        return formatMessage(themeSuiteMessages.permissionInstancesLogs)
    }
    return formatMessage(themeSuiteMessages.permissionNetwork)
}

function requestRemove(pack: InstalledThemePack) {
    confirmingRemove.value = pack.manifest.id
    removeData.value = false
}

function cancelRemove() {
    confirmingRemove.value = null
    removeData.value = false
}

function confirmRemove(pack: InstalledThemePack) {
    confirmingRemove.value = null
    void remove(pack, removeData.value)
    removeData.value = false
}
</script>

<template>
    <div class="flex min-w-0 flex-col gap-4">
        <p class="m-0 text-sm text-[var(--color-text-tertiary)]">
            {{ formatMessage(themeSuiteMessages.description) }}
        </p>

        <div
            v-if="loading"
            class="flex items-center gap-2 text-sm text-[var(--color-text-tertiary)]"
            role="status"
        >
            <SpinnerIcon class="size-4 animate-spin" aria-hidden="true" />
            {{ formatMessage(themeSuiteMessages.loading) }}
        </div>

        <div v-if="error" class="flex flex-col gap-1" role="alert">
            <strong v-if="!settings" class="text-sm text-[var(--color-red)]">
                {{ formatMessage(themeSuiteMessages.loadFailed) }}
            </strong>
            <p class="m-0 break-all text-sm text-[var(--color-red)]">{{ error }}</p>
        </div>

        <section class="flex flex-col gap-3">
            <div class="flex flex-col gap-1">
                <h3 class="m-0 text-base font-semibold text-[var(--color-text-primary)]">
                    {{ formatMessage(themeSuiteMessages.overviewTitle) }}
                </h3>
                <p class="m-0 text-sm text-[var(--color-text-tertiary)]">
                    {{ formatMessage(themeSuiteMessages.overviewDescription) }}
                </p>
            </div>

            <div
                role="radiogroup"
                :aria-label="formatMessage(themeSuiteMessages.overviewTitle)"
                class="flex flex-col gap-2"
            >
                <div
                    class="flex items-start gap-2 rounded-xl border border-solid p-3"
                    :class="activeSource === 'builtin' ? 'border-brand' : 'border-surface-4'"
                >
                    <button
                        type="button"
                        role="radio"
                        :aria-checked="activeSource === 'builtin'"
                        class="flex min-w-0 flex-1 items-start gap-3 text-left"
                        @click="selectSource('builtin')"
                    >
                        <span
                            class="mt-0.5 flex size-4 shrink-0 items-center justify-center rounded-full border border-solid"
                            :class="
                                activeSource === 'builtin' ? 'border-brand' : 'border-surface-5'
                            "
                        >
                            <span
                                v-if="activeSource === 'builtin'"
                                class="size-2 rounded-full bg-brand"
                            />
                        </span>
                        <span class="flex min-w-0 flex-col gap-1">
                            <span class="font-semibold text-[var(--color-text-primary)]">
                                {{ formatMessage(themeSuiteMessages.builtInName) }}
                            </span>
                            <span class="text-sm text-[var(--color-text-tertiary)]">
                                {{ formatMessage(themeSuiteMessages.builtInDescription) }}
                            </span>
                        </span>
                    </button>
                    <span
                        v-if="activeSource === 'builtin'"
                        class="shrink-0 rounded-full bg-brand-highlight px-2 py-0.5 text-xs font-semibold text-brand"
                    >
                        {{ formatMessage(themeSuiteMessages.activeBadge) }}
                    </span>
                    <Button
                        v-else
                        size="sm"
                        :loading="selectingSource === 'builtin'"
                        @click="selectSource('builtin')"
                    >
                        {{ formatMessage(themeSuiteMessages.activate) }}
                    </Button>
                </div>

                <p
                    v-if="packs.length === 0"
                    class="m-0 rounded-xl border border-dashed border-surface-4 p-3 text-sm text-[var(--color-text-tertiary)]"
                >
                    {{ formatMessage(themeSuiteMessages.noPacks) }}
                </p>

                <template v-else>
                    <span
                        class="pt-1 text-xs font-semibold uppercase tracking-wide text-[var(--color-text-tertiary)]"
                    >
                        {{ formatMessage(themeSuiteMessages.installedTitle) }}
                    </span>
                    <div
                        v-for="pack in packs"
                        :key="pack.manifest.id"
                        class="rounded-xl border border-solid"
                        :class="
                            activeSource === `pack:${pack.manifest.id}`
                                ? 'border-brand'
                                : 'border-surface-4'
                        "
                    >
                        <div class="flex items-start gap-2 p-3">
                            <button
                                type="button"
                                role="radio"
                                :aria-checked="activeSource === `pack:${pack.manifest.id}`"
                                class="flex min-w-0 flex-1 items-start gap-3 text-left"
                                @click="selectSource(`pack:${pack.manifest.id}`)"
                            >
                                <span
                                    class="mt-0.5 flex size-4 shrink-0 items-center justify-center rounded-full border border-solid"
                                    :class="
                                        activeSource === `pack:${pack.manifest.id}`
                                            ? 'border-brand'
                                            : 'border-surface-5'
                                    "
                                >
                                    <span
                                        v-if="activeSource === `pack:${pack.manifest.id}`"
                                        class="size-2 rounded-full bg-brand"
                                    />
                                </span>
                                <span class="flex min-w-0 flex-col gap-1">
                                    <span class="font-semibold text-[var(--color-text-primary)]">
                                        {{ packName(pack) }}
                                    </span>
                                    <span
                                        v-if="packDescription(pack)"
                                        class="text-sm text-[var(--color-text-tertiary)]"
                                    >
                                        {{ packDescription(pack) }}
                                    </span>
                                    <span class="text-xs text-[var(--color-text-tertiary)]">
                                        {{
                                            formatMessage(themeSuiteMessages.versionLabel, {
                                                version: pack.manifest.version,
                                            })
                                        }}
                                        ·
                                        {{
                                            formatMessage(themeSuiteMessages.sizeLabel, {
                                                size: formatBytes(pack.sizeBytes),
                                            })
                                        }}
                                        ·
                                        {{
                                            formatMessage(themeSuiteMessages.engineLabel, {
                                                version: pack.manifest.engines.axolotl,
                                            })
                                        }}
                                    </span>
                                    <span class="flex flex-wrap gap-1 pt-1">
                                        <span
                                            v-for="capability in pack.manifest.capabilities"
                                            :key="capability"
                                            class="rounded-full bg-surface-4 px-2 py-0.5 text-xs text-[var(--color-text-tertiary)]"
                                        >
                                            {{ capabilityLabel(capability) }}
                                        </span>
                                    </span>
                                    <span
                                        v-if="pack.manifest.permissions.length > 0"
                                        class="text-xs text-[var(--color-text-tertiary)]"
                                    >
                                        {{ formatMessage(themeSuiteMessages.packPermissions) }}:
                                        {{
                                            pack.manifest.permissions
                                                .map(permissionLabel)
                                                .join(', ')
                                        }}
                                    </span>
                                </span>
                            </button>
                            <div class="flex shrink-0 items-center gap-1">
                                <span
                                    v-if="activeSource === `pack:${pack.manifest.id}`"
                                    class="rounded-full bg-brand-highlight px-2 py-0.5 text-xs font-semibold text-brand"
                                >
                                    {{ formatMessage(themeSuiteMessages.activeBadge) }}
                                </span>
                                <Button
                                    v-else
                                    size="sm"
                                    :loading="selectingSource === `pack:${pack.manifest.id}`"
                                    @click="selectSource(`pack:${pack.manifest.id}`)"
                                >
                                    {{ formatMessage(themeSuiteMessages.activate) }}
                                </Button>
                                <Button
                                    type="outlined"
                                    color="red"
                                    size="sm"
                                    :loading="removingId === pack.manifest.id"
                                    @click="requestRemove(pack)"
                                >
                                    {{ formatMessage(themeSuiteMessages.removePack) }}
                                </Button>
                            </div>
                        </div>
                        <div
                            v-if="confirmingRemove === pack.manifest.id"
                            class="flex flex-col gap-2 border-0 border-t border-solid border-surface-4 p-3"
                        >
                            <p class="m-0 text-sm text-[var(--color-text-primary)]">
                                {{
                                    formatMessage(themeSuiteMessages.removePackConfirm, {
                                        name: packName(pack),
                                    })
                                }}
                            </p>
                            <label
                                class="flex items-center gap-2 text-sm text-[var(--color-text-tertiary)]"
                            >
                                <input v-model="removeData" type="checkbox" />
                                {{ formatMessage(themeSuiteMessages.removePackData) }}
                            </label>
                            <div class="flex flex-wrap gap-2">
                                <Button
                                    type="outlined"
                                    color="red"
                                    size="sm"
                                    :loading="removingId === pack.manifest.id"
                                    @click="confirmRemove(pack)"
                                >
                                    {{ formatMessage(themeSuiteMessages.removePack) }}
                                </Button>
                                <Button size="sm" @click="cancelRemove">
                                    {{ formatMessage(themeSuiteMessages.removePackCancel) }}
                                </Button>
                            </div>
                        </div>
                    </div>
                </template>
            </div>
        </section>

        <section class="flex flex-col gap-2 border-0 border-t border-solid border-surface-4 pt-3">
            <div class="flex flex-wrap items-center gap-2">
                <Button @click="openImport">
                    {{ formatMessage(themeSuiteMessages.importPack) }}
                </Button>
                <Button
                    v-tooltip="formatMessage(themeSuiteMessages.customColorsDescription)"
                    @click="openColors"
                >
                    {{ formatMessage(themeSuiteMessages.customColors) }}
                </Button>
                <span
                    v-if="activeSource === 'custom'"
                    class="rounded-full bg-brand-highlight px-2 py-0.5 text-xs font-semibold text-brand"
                >
                    {{ formatMessage(themeSuiteMessages.activeBadge) }}
                </span>
                <Button :loading="snapshotExporting" @click="exportSnapshot">
                    {{
                        formatMessage(
                            snapshotExporting
                                ? themeSuiteMessages.exportingSnapshot
                                : themeSuiteMessages.exportSnapshot,
                        )
                    }}
                </Button>
            </div>
            <p
                v-if="snapshotExported && !snapshotError"
                class="m-0 text-sm text-green"
                role="status"
            >
                {{ formatMessage(themeSuiteMessages.exportSnapshotDone) }}
            </p>
            <p
                v-if="snapshotError"
                class="m-0 break-all text-sm text-[var(--color-red)]"
                role="alert"
            >
                {{
                    formatMessage(themeSuiteMessages.exportSnapshotFailed, { error: snapshotError })
                }}
            </p>
        </section>
    </div>
</template>
