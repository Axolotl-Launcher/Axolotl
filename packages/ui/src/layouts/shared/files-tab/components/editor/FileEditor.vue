<template>
    <div
        ref="editorContainer"
        class="relative flex flex-col overflow-hidden rounded-[var(--radius-xl)] border border-solid border-surface-4 shadow-sm"
    >
        <EditorFindReplace
            ref="findReplaceRef"
            v-model:is-find-open="isFindOpen"
            v-model:find-query="inFileFindQuery"
            :is-editing-image="isEditingImage"
            :readonly="isEditorReadOnly"
            :find-match-count="findMatchCount"
            :current-find-match="currentFindMatch"
            @find-next="findNext"
            @find-previous="findPrevious"
            @close="closeFind"
            @replace="replaceOne"
            @replace-all="replaceAllOccurrences"
        />
        <CodeEditor
            v-if="!isEditingImage && !isLoading && !editorFailed"
            v-model:content="fileContent"
            :file-path="props.file?.path"
            :language="editorLanguage"
            :read-only="isEditorReadOnly"
            :options="{ minimap: { enabled: false }, wordWrap: 'on' }"
            :style="{ height: editorHeight }"
            @ready="onEditorInit"
            @save="saveFileContent(false)"
            @error="editorFailed = true"
        />
        <FileImageViewer v-else-if="isEditingImage && imagePreview" :image-blob="imagePreview" />
        <textarea
            v-else-if="!isEditingImage && !isLoading"
            :value="fileContent"
            :readonly="isEditorReadOnly"
            :placeholder="formatMessage(messages.editorUnavailablePlaceholder)"
            class="w-full resize-none rounded-[var(--radius-xl)] bg-surface-3 p-3 font-mono text-sm text-[var(--color-text-default)] outline-none"
            :style="{ height: editorHeight }"
            @input="onFallbackInput"
        />
        <div
            v-else-if="isLoading"
            class="flex items-center justify-center rounded-[var(--radius-xl)] bg-surface-3"
            :style="{ height: editorHeight }"
        >
            <SpinnerIcon class="h-8 w-8 animate-spin text-[var(--color-text-tertiary)]" />
        </div>
    </div>
</template>

<script setup lang="ts">
import { SpinnerIcon } from '@modrinth/assets'
import type * as Monaco from 'monaco-editor/editor/editor.api.js'
import { computed, nextTick, onMounted, onUnmounted, ref, shallowRef, watch } from 'vue'

import CodeEditor from '#ui/components/code-editor/CodeEditor.vue'
import { defineMessages, useVIntl } from '#ui/composables/i18n'
import { injectModrinthClient } from '#ui/providers'
import { injectNotificationManager } from '#ui/providers/web-notifications'
import { getEditorLanguage, getFileExtension, isImageFile } from '#ui/utils/file-extensions'
import { shareLogs } from '#ui/utils/log-share'

import { injectFileManager } from '../../providers/file-manager'
import type { EditingFile } from '../../types'
import EditorFindReplace from './EditorFindReplace.vue'
import FileImageViewer from './FileImageViewer.vue'

const props = defineProps<{
    file: EditingFile | null
}>()

const emit = defineEmits<{
    close: []
}>()

const { formatMessage } = useVIntl()
const { addNotification } = injectNotificationManager()
const ctx = injectFileManager()
const client = injectModrinthClient()

const messages = defineMessages({
    failedToOpenTitle: {
        id: 'files.editor.failed-to-open-title',
        defaultMessage: 'Failed to open file',
    },
    failedToOpenText: {
        id: 'files.editor.failed-to-open-text',
        defaultMessage: 'Could not load file contents.',
    },
    fileSavedTitle: {
        id: 'files.editor.file-saved-title',
        defaultMessage: 'File saved',
    },
    fileSavedText: {
        id: 'files.editor.file-saved-text',
        defaultMessage: 'Your file has been saved.',
    },
    saveFailedTitle: {
        id: 'files.editor.save-failed-title',
        defaultMessage: 'Save failed',
    },
    saveFailedText: {
        id: 'files.editor.save-failed-text',
        defaultMessage: 'Could not save the file.',
    },
    logUrlCopiedTitle: {
        id: 'files.editor.log-url-copied-title',
        defaultMessage: 'Log URL copied',
    },
    logUrlCopiedText: {
        id: 'files.editor.log-url-copied-text',
        defaultMessage: 'Your log file URL has been copied to your clipboard.',
    },
    failedToShareTitle: {
        id: 'files.editor.failed-to-share-title',
        defaultMessage: 'Failed to share file',
    },
    failedToShareText: {
        id: 'files.editor.failed-to-share-text',
        defaultMessage: 'Could not share the log file.',
    },
    logTruncatedWarning: {
        id: 'files.editor.share-truncated-warning',
        defaultMessage: 'The log file is too large, so only the last 9 MB was uploaded.',
    },
    editorUnavailablePlaceholder: {
        id: 'files.editor.editor-unavailable-placeholder',
        defaultMessage: 'Code editor unavailable',
    },
})

const fileContent = ref('')
const originalContent = ref('')
const isEditingImage = ref(false)
const imagePreview = ref<Blob | null>(null)
const isLoading = ref(false)
// Only the latest load may fill the editor or end its loading state; an
// earlier read finishing first must not expose the editor for the new file.
let loadRequest = 0
const saving = ref(false)
const editorInstance = shallowRef<Monaco.editor.IStandaloneCodeEditor | null>(null)
const editorFailed = ref(false)
const editorContainer = ref<HTMLElement | null>(null)
const editorHeight = ref('300px')

const isFindOpen = ref(false)
const inFileFindQuery = ref('')
const findMatchCount = ref(0)
const currentFindMatch = ref(0)
const findReplaceRef = ref<{ focusFindInput: () => void; openReplace: () => void } | null>(null)

watch(inFileFindQuery, handleFindInput)
watch(
    fileContent,
    () => {
        const count = findMatches().length
        findMatchCount.value = count
        currentFindMatch.value = count ? Math.min(currentFindMatch.value || 1, count) : 0
    },
    { flush: 'post' },
)

function updateEditorHeight() {
    if (editorContainer.value) {
        const top = editorContainer.value.getBoundingClientRect().top
        const padding = 24
        editorHeight.value = `${Math.max(300, window.innerHeight - top - padding)}px`
    }
}

onMounted(() => {
    nextTick(updateEditorHeight)
    window.addEventListener('resize', updateEditorHeight)
})

const editorLanguage = computed(() => {
    const ext = getFileExtension(props.file?.name ?? '')
    return getEditorLanguage(ext)
})
const isEditorReadOnly = computed(() => ctx.isBusy?.value ?? false)

watch(isLoading, (loading) => {
    if (loading) editorInstance.value = null
})

watch(
    () => props.file,
    async (newFile) => {
        if (newFile) {
            closeFind()
            await loadFileContent(newFile)
            nextTick(updateEditorHeight)
        } else {
            resetState()
        }
    },
    { immediate: true },
)

async function loadFileContent(file: { name: string; path: string }) {
    const request = ++loadRequest
    isLoading.value = true
    try {
        window.scrollTo(0, 0)
        const extension = getFileExtension(file.name)
        const normalizedPath = file.path.startsWith('/') ? file.path : `/${file.path}`

        if (isImageFile(extension)) {
            const content = await ctx.readFileAsBlob(normalizedPath)
            if (request !== loadRequest) return
            isEditingImage.value = true
            imagePreview.value = content
        } else {
            isEditingImage.value = false
            const content = await ctx.readFile(normalizedPath)
            if (request !== loadRequest) return
            fileContent.value = content
            originalContent.value = content
        }
    } catch (error) {
        if (request !== loadRequest) return
        console.error('Error fetching file content:', error)
        addNotification({
            title: formatMessage(messages.failedToOpenTitle),
            text: formatMessage(messages.failedToOpenText),
            type: 'error',
        })
        emit('close')
    } finally {
        if (request === loadRequest) isLoading.value = false
    }
}

const hasUnsavedChanges = computed(
    () => !isEditingImage.value && !isLoading.value && fileContent.value !== originalContent.value,
)

function revertChanges() {
    fileContent.value = originalContent.value
}

function onFallbackInput(event: Event) {
    fileContent.value = (event.target as HTMLTextAreaElement).value
}

function resetState() {
    fileContent.value = ''
    originalContent.value = ''
    isEditingImage.value = false
    imagePreview.value = null
}

function onEditorInit(editor: Monaco.editor.IStandaloneCodeEditor, runtime: typeof Monaco) {
    editorInstance.value = editor
    editor.addCommand(runtime.KeyMod.CtrlCmd | runtime.KeyCode.KeyF, () => toggleFind())
    const replaceKey = /Mac/.test(navigator.platform)
        ? runtime.KeyMod.CtrlCmd | runtime.KeyMod.Alt | runtime.KeyCode.KeyF
        : runtime.KeyMod.CtrlCmd | runtime.KeyCode.KeyH
    editor.addCommand(replaceKey, () => {
        if (isEditorReadOnly.value) return
        isFindOpen.value = true
        nextTick(() => findReplaceRef.value?.openReplace())
    })
    handleFindInput()
}

let pendingSave: Promise<boolean> | null = null

/**
 * Writes the text as it was when saving started and resolves whether that
 * write succeeded. Only that snapshot is marked as saved, so input typed while
 * the write was pending stays unsaved. A save requested while another one is
 * writing waits for it and then writes only what is still unsaved.
 */
async function saveFileContent(exit: boolean = false): Promise<boolean> {
    if (pendingSave) {
        const saved = await pendingSave
        if (!saved) return false
        if (pendingSave != null) return saveFileContent(exit)
        if (!hasUnsavedChanges.value) {
            if (exit) emit('close')
            return true
        }
    }
    const file = props.file
    if (!file || ctx.isBusy?.value) return false
    const save = writeSnapshot(file, exit)
    pendingSave = save
    try {
        return await save
    } finally {
        if (pendingSave === save) pendingSave = null
    }
}

async function writeSnapshot(file: EditingFile, exit: boolean): Promise<boolean> {
    const normalizedPath = file.path.startsWith('/') ? file.path : `/${file.path}`
    const snapshot = fileContent.value
    saving.value = true
    try {
        await ctx.writeFile(normalizedPath, snapshot)
    } catch (error) {
        console.error('Error saving file content:', error)
        addNotification({
            title: formatMessage(messages.saveFailedTitle),
            text: formatMessage(messages.saveFailedText),
            type: 'error',
        })
        return false
    } finally {
        saving.value = false
    }

    if (props.file?.path === file.path) {
        originalContent.value = snapshot
        if (exit && fileContent.value === snapshot) emit('close')
    }
    addNotification({
        title: formatMessage(messages.fileSavedTitle),
        text: formatMessage(messages.fileSavedText),
        type: 'success',
    })
    return true
}

async function shareLog() {
    if (ctx.shareLogs) {
        await ctx.shareLogs(fileContent.value)
        return
    }

    try {
        const result = await shareLogs(client, fileContent.value)

        if (result.url) {
            if (result.truncated) {
                addNotification({
                    title: formatMessage(messages.logTruncatedWarning),
                    type: 'warning',
                })
            }
            await navigator.clipboard.writeText(result.url)
            addNotification({
                title: formatMessage(messages.logUrlCopiedTitle),
                text: formatMessage(messages.logUrlCopiedText),
                type: 'success',
            })
        } else {
            throw new Error('log share failed')
        }
    } catch (error) {
        console.error('Error sharing file:', error)
        addNotification({
            title: formatMessage(messages.failedToShareTitle),
            text: formatMessage(messages.failedToShareText),
            type: 'error',
        })
    }
}

function findMatches() {
    const query = inFileFindQuery.value
    return query
        ? (editorInstance.value
              ?.getModel()
              ?.findMatches(query, false, false, false, null, false, Number.MAX_SAFE_INTEGER) ?? [])
        : []
}

function selectMatch(index: number) {
    const matches = findMatches()
    findMatchCount.value = matches.length
    if (!matches.length) {
        currentFindMatch.value = 0
        return
    }
    const normalized = ((index % matches.length) + matches.length) % matches.length
    currentFindMatch.value = normalized + 1
    const range = matches[normalized].range
    editorInstance.value?.setSelection(range)
    editorInstance.value?.revealRangeInCenterIfOutsideViewport(range)
}

function toggleFind() {
    if (isFindOpen.value) {
        closeFind()
    } else {
        isFindOpen.value = true
        nextTick(() => findReplaceRef.value?.focusFindInput())
    }
}

function closeFind() {
    isFindOpen.value = false
    inFileFindQuery.value = ''
    findMatchCount.value = 0
    currentFindMatch.value = 0
    editorInstance.value?.focus()
}

function replaceOne(replacement: string) {
    const editor = editorInstance.value
    if (!editor || isEditorReadOnly.value) return
    const matches = findMatches()
    const index = Math.max(0, currentFindMatch.value - 1)
    const match = matches[index]
    if (!match) return
    editor.pushUndoStop()
    editor.executeEdits('replace', [
        { range: match.range, text: replacement, forceMoveMarkers: true },
    ])
    editor.pushUndoStop()
    selectMatch(index)
}

function replaceAllOccurrences(replacement: string) {
    const editor = editorInstance.value
    if (!editor || isEditorReadOnly.value) return
    const matches = findMatches()
    if (!matches.length) return
    editor.pushUndoStop()
    editor.executeEdits(
        'replace-all',
        matches.map(({ range }) => ({ range, text: replacement, forceMoveMarkers: true })),
    )
    editor.pushUndoStop()
    selectMatch(0)
}

function handleFindInput() {
    selectMatch(0)
}

function findNext() {
    selectMatch(currentFindMatch.value)
}

function findPrevious() {
    selectMatch(currentFindMatch.value - 2)
}

function close() {
    resetState()
    emit('close')
}

onUnmounted(() => {
    window.removeEventListener('resize', updateEditorHeight)
    editorInstance.value = null
    resetState()
})

defineExpose({
    saveFileContent,
    shareLog,
    close,
    isEditingImage,
    isFindOpen,
    fileContent,
    hasUnsavedChanges,
    saving,
    revertChanges,
    toggleFind,
})
</script>
