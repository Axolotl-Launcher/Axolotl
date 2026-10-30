<script setup lang="ts">
import { DownloadIcon, FileArchiveIcon } from '@modrinth/assets'
import { Button, defineMessages, NewModal, useVIntl } from '@modrinth/ui'
import { isTauri } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import { readTextFile, stat } from '@tauri-apps/plugin-fs'
import { computed, ref, useTemplateRef } from 'vue'

import {
    BannerParseError,
    MAX_IMPORT_BYTES,
    parseBannerText,
} from '@/lab/banner-designer/serialization.ts'
import type { BannerDesign } from '@/lab/banner-designer/types.ts'

const emit = defineEmits<{
    import: [design: BannerDesign]
}>()

const { formatMessage } = useVIntl()
const modal = useTemplateRef<InstanceType<typeof NewModal>>('modal')

const messages = defineMessages({
    title: { id: 'app.lab.banner-designer.import.title', defaultMessage: 'Import banner' },
    description: {
        id: 'app.lab.banner-designer.import.description',
        defaultMessage: 'Paste a give command, item data or a design file, or open a design file.',
    },
    placeholder: {
        id: 'app.lab.banner-designer.import.placeholder',
        defaultMessage: '/give @p minecraft:white_banner[minecraft:banner_patterns=[…]] 1',
    },
    chooseFile: {
        id: 'app.lab.banner-designer.import.choose-file',
        defaultMessage: 'Open file',
    },
    cancel: { id: 'app.lab.banner-designer.import.cancel', defaultMessage: 'Cancel' },
    confirm: { id: 'app.lab.banner-designer.import.confirm', defaultMessage: 'Import' },
    errorEmpty: {
        id: 'app.lab.banner-designer.import.error.empty',
        defaultMessage: 'Paste a give command, item data or design JSON first.',
    },
    errorUnsupported: {
        id: 'app.lab.banner-designer.import.error.unsupported',
        defaultMessage: 'This text is not a banner give command, item data or design file.',
    },
    errorColor: {
        id: 'app.lab.banner-designer.import.error.color',
        defaultMessage: 'Unknown dye color: {value}',
    },
    errorPattern: {
        id: 'app.lab.banner-designer.import.error.pattern',
        defaultMessage: 'Unknown banner pattern: {value}',
    },
    errorData: {
        id: 'app.lab.banner-designer.import.error.data',
        defaultMessage: 'The item data could not be read. Check that it is complete.',
    },
    errorTooLarge: {
        id: 'app.lab.banner-designer.import.error.too-large',
        defaultMessage:
            'This file is far larger than a banner design. Check that you picked the right one.',
    },
    fileFailed: {
        id: 'app.lab.banner-designer.import.error.file',
        defaultMessage: 'The file could not be read.',
    },
})

const text = ref('')
const error = ref<string | null>(null)
const showFileButton = computed(() => isTauri())

/**
 * Identifies the file read the dialog is currently waiting on. Closing the
 * dialog or asking for another file bumps it, so a read that finishes later
 * cannot import into a session the user already left.
 */
let fileRequest = 0

function dropPendingFile() {
    fileRequest += 1
}

function describeError(cause: unknown): string {
    if (!(cause instanceof BannerParseError)) {
        return formatMessage(messages.errorData)
    }
    if (cause.code === 'empty') return formatMessage(messages.errorEmpty)
    if (cause.code === 'unsupported') return formatMessage(messages.errorUnsupported)
    if (cause.code === 'too-large') return formatMessage(messages.errorTooLarge)
    if (cause.code === 'invalid-color') {
        return formatMessage(messages.errorColor, { value: cause.detail ?? '' })
    }
    if (cause.code === 'invalid-pattern') {
        return formatMessage(messages.errorPattern, { value: cause.detail ?? '' })
    }
    return formatMessage(messages.errorData)
}

function show() {
    dropPendingFile()
    text.value = ''
    error.value = null
    modal.value?.show()
}

function submit() {
    try {
        const design = parseBannerText(text.value)
        error.value = null
        emit('import', design)
        modal.value?.hide()
    } catch (cause) {
        error.value = describeError(cause)
    }
}

async function chooseFile() {
    const request = ++fileRequest
    try {
        const selected = await open({
            multiple: false,
            filters: [{ name: 'Banner design', extensions: ['json', 'txt', 'snbt'] }],
        })
        if (typeof selected !== 'string' || request !== fileRequest) return
        // Reject on the reported size so an oversized file is never read in.
        const info = await stat(selected)
        if (request !== fileRequest) return
        if (info.size > MAX_IMPORT_BYTES) {
            error.value = formatMessage(messages.errorTooLarge)
            return
        }
        const content = await readTextFile(selected)
        if (request !== fileRequest) return
        text.value = content
        error.value = null
        submit()
    } catch {
        if (request !== fileRequest) return
        error.value = formatMessage(messages.fileFailed)
    }
}

defineExpose({ show })
</script>

<template>
    <NewModal
        ref="modal"
        :header="formatMessage(messages.title)"
        width="min(44rem, calc(95vw - 10rem))"
        max-width="44rem"
        :on-hide="dropPendingFile"
    >
        <div class="flex flex-col gap-3">
            <p class="m-0 text-sm text-[var(--color-text-tertiary)]">
                {{ formatMessage(messages.description) }}
            </p>

            <textarea
                v-model="text"
                rows="6"
                spellcheck="false"
                :placeholder="formatMessage(messages.placeholder)"
                :aria-label="formatMessage(messages.description)"
                class="w-full resize-y rounded-[var(--radius-md)] border border-surface-5 bg-surface-1 p-3 font-mono text-xs leading-relaxed text-[var(--color-text-default)] outline-none transition-colors focus:border-brand"
                @input="error = null"
            ></textarea>

            <p v-if="error" class="m-0 text-sm text-brand-red" role="alert">{{ error }}</p>
        </div>

        <template #actions>
            <div class="flex items-center justify-between gap-2">
                <Button v-if="showFileButton" type="outlined" @click="chooseFile"
                    ><FileArchiveIcon />{{ formatMessage(messages.chooseFile) }}
                </Button>
                <span v-else></span>
                <div class="flex items-center gap-2">
                    <Button type="quiet" @click="modal?.hide()">{{
                        formatMessage(messages.cancel)
                    }}</Button>
                    <Button type="colored" color="brand" :disabled="!text.trim()" @click="submit"
                        ><DownloadIcon />{{ formatMessage(messages.confirm) }}
                    </Button>
                </div>
            </div>
        </template>
    </NewModal>
</template>
