<script setup lang="ts">
import { DownloadIcon, SearchIcon, SpinnerIcon } from '@modrinth/assets'
import { Button, defineMessages, NewModal, SkinPreviewRenderer, useVIntl } from '@modrinth/ui'
import { save } from '@tauri-apps/plugin-dialog'
import { writeFile } from '@tauri-apps/plugin-fs'
import { computed, onBeforeUnmount, ref, useTemplateRef, watch } from 'vue'

import { lookup_player_skin, type PlayerSkin, save_custom_skin, type Skin } from '@/helpers/skins'

const props = defineProps<{ readOnly?: boolean; savedSkins?: Skin[] }>()
const emit = defineEmits<{ saved: [options: { applied: boolean; skin: Skin }] }>()
const { formatMessage } = useVIntl()
const messages = defineMessages({
    title: { id: 'app.skins.player.title', defaultMessage: 'Find player skin' },
    input: { id: 'app.skins.player.input', defaultMessage: 'Player name or UUID' },
    search: { id: 'app.skins.player.search', defaultMessage: 'Search' },
    download: { id: 'app.skins.player.download', defaultMessage: 'Download PNG' },
    import: { id: 'app.skins.player.import', defaultMessage: 'Add to saved skins' },
    imported: { id: 'app.skins.player.imported', defaultMessage: 'Added to saved skins' },
    invalid_input: {
        id: 'app.skins.player.invalid-input',
        defaultMessage: 'Enter a valid Java player name or UUID.',
    },
    not_found: {
        id: 'app.skins.player.not-found',
        defaultMessage: 'Player not found. Check the name or UUID.',
    },
    no_custom_skin: {
        id: 'app.skins.player.no-custom-skin',
        defaultMessage: 'This player has no custom skin.',
    },
    rate_limited: {
        id: 'app.skins.player.rate-limited',
        defaultMessage: 'Too many requests. Please wait before trying again.',
    },
    unavailable: {
        id: 'app.skins.player.unavailable',
        defaultMessage: 'Could not load this skin. Check your connection and try again.',
    },
    saveFailed: {
        id: 'app.skins.player.save-failed',
        defaultMessage: 'Could not save the skin. Please try again.',
    },
})
const modal = useTemplateRef('modal')
const input = ref('')
const result = ref<PlayerSkin | null>(null)
const busy = ref(false)
const saving = ref(false)
const imported = ref(false)
const error = ref('')
let generation = 0
const bytes = computed(() =>
    result.value
        ? Uint8Array.from(atob(result.value.texture.split(',')[1]), (char) => char.charCodeAt(0))
        : null,
)

function reset() {
    generation++
    result.value = null
    error.value = ''
    busy.value = false
    imported.value = false
}
watch(input, () => {
    generation++
    result.value = null
    error.value = ''
    imported.value = false
})
onBeforeUnmount(() => generation++)

function show(event?: MouseEvent) {
    reset()
    input.value = ''
    modal.value?.show(event)
}
function hide() {
    reset()
    modal.value?.hide()
}

async function search() {
    if (busy.value || saving.value || !input.value.trim()) return
    const request = ++generation
    result.value = null
    error.value = ''
    imported.value = false
    busy.value = true
    try {
        const response = await lookup_player_skin(input.value.trim())
        if (request !== generation) return
        if (response.status === 'found') result.value = response
        else error.value = formatMessage(messages[response.status])
    } catch {
        if (request === generation) error.value = formatMessage(messages.unavailable)
    } finally {
        if (request === generation) busy.value = false
    }
}

async function download() {
    if (!result.value || !bytes.value || saving.value) return
    const player = result.value
    const png = bytes.value
    saving.value = true
    error.value = ''
    try {
        const path = await save({
            defaultPath: `${player.name}.png`,
            filters: [{ name: 'PNG', extensions: ['png'] }],
        })
        if (path) await writeFile(path, png)
    } catch {
        error.value = formatMessage(messages.saveFailed)
    } finally {
        saving.value = false
    }
}

async function importSkin() {
    if (!result.value || !bytes.value || props.readOnly || saving.value || imported.value) return
    const player = result.value
    saving.value = true
    error.value = ''
    try {
        const existing = props.savedSkins?.find((skin) => skin.texture === player.texture)
        const skin =
            existing ??
            (await save_custom_skin(
                {
                    texture_key: '',
                    variant: player.variant,
                    texture: player.texture,
                    source: 'custom',
                    is_equipped: false,
                },
                bytes.value,
                player.variant,
                undefined,
                true,
            ))
        imported.value = true
        emit('saved', { applied: false, skin })
    } catch {
        error.value = formatMessage(messages.saveFailed)
    } finally {
        saving.value = false
    }
}

defineExpose({ show, hide })
</script>

<template>
    <NewModal
        ref="modal"
        :header="formatMessage(messages.title)"
        width="min(34rem, 90vw)"
        :disable-close="saving"
        :on-hide="reset"
    >
        <div class="flex flex-col gap-4">
            <form class="flex flex-col gap-2" @submit.prevent="search">
                <label for="player-skin-query" class="font-semibold">{{
                    formatMessage(messages.input)
                }}</label>
                <div class="flex gap-2">
                    <input
                        id="player-skin-query"
                        v-model="input"
                        maxlength="36"
                        autocomplete="off"
                        spellcheck="false"
                        :disabled="busy || saving"
                        class="min-w-0 flex-1 rounded-lg border border-divider bg-surface-1 px-3 py-2 text-[var(--color-text-primary)]"
                    />
                    <Button :disabled="busy || saving || !input.trim()" @click="search">
                        <SpinnerIcon v-if="busy" class="animate-spin" /><SearchIcon v-else />
                        {{ formatMessage(messages.search) }}
                    </Button>
                </div>
            </form>
            <p v-if="error" role="alert" class="m-0 text-red">{{ error }}</p>
            <div v-if="result" class="flex flex-col items-center gap-2">
                <div class="h-64 w-full">
                    <SkinPreviewRenderer
                        :texture-src="result.preview_texture"
                        :variant="result.variant"
                        framing="modal"
                    />
                </div>
                <strong>{{ result.name }}</strong>
                <span class="break-all text-sm text-[var(--color-text-tertiary)]">{{
                    result.uuid
                }}</span>
                <p v-if="imported" role="status" class="m-0">
                    {{ formatMessage(messages.imported) }}
                </p>
            </div>
        </div>
        <template #actions>
            <div class="flex flex-wrap justify-end gap-2">
                <Button :disabled="!result || saving" @click="download"
                    ><DownloadIcon />{{ formatMessage(messages.download) }}</Button
                >
                <Button
                    type="colored"
                    color="brand"
                    :disabled="!result || saving || readOnly || imported"
                    @click="importSkin"
                >
                    <SpinnerIcon v-if="saving" class="animate-spin" />{{
                        formatMessage(messages.import)
                    }}
                </Button>
            </div>
        </template>
    </NewModal>
</template>
