import { computed, ref } from 'vue'

export interface StudioDocument {
    kind: 'text' | 'nbt' | 'image' | 'video' | 'unsupported'
    path: string
    name: string
    content: string
    savedContent: string
    saving: boolean
}

export function useStudioDocuments(
    writeDocument: (document: StudioDocument, content: string) => Promise<void>,
    onSaveError: (error: unknown) => void,
) {
    const documents = ref<StudioDocument[]>([])
    const activeIndex = ref(-1)
    const savePromises = new Map<string, Promise<boolean>>()
    const deletingPaths = ref(new Set<string>())
    const pathVersions = new Map<string, number>()
    let generation = 0

    function containsPath(parent: string, path: string) {
        return parent === path || path.startsWith(`${parent}/`)
    }

    function isDeleting(path: string) {
        return [...deletingPaths.value].some((parent) => containsPath(parent, path))
    }

    function pathVersion(path: string) {
        const version = [...pathVersions]
            .filter(([parent]) => containsPath(parent, path))
            .reduce((version, [, value]) => version + value, 0)
        return { generation, version }
    }

    function canOpen(path: string, token: ReturnType<typeof pathVersion>) {
        const current = pathVersion(path)
        return (
            !isDeleting(path) &&
            current.generation === token.generation &&
            current.version === token.version
        )
    }

    const activeDocument = computed(() => documents.value[activeIndex.value] ?? null)
    const activePath = computed(() => activeDocument.value?.path ?? '')
    const hasUnsavedChanges = computed(
        () =>
            activeDocument.value !== null &&
            activeDocument.value.content !== activeDocument.value.savedContent,
    )
    const hasAnyUnsavedChanges = computed(() =>
        documents.value.some((document) => document.content !== document.savedContent),
    )

    function saveDocument(document: StudioDocument | null): Promise<boolean> {
        if (document && (isDeleting(document.path) || !documents.value.includes(document))) {
            return Promise.resolve(false)
        }
        if (
            !document ||
            (document.kind !== 'text' && document.kind !== 'nbt') ||
            document.content === document.savedContent
        ) {
            return Promise.resolve(true)
        }

        const existingPromise = savePromises.get(document.path)
        if (existingPromise) return existingPromise

        document.saving = true
        const contentToSave = document.content
        const savePromise = writeDocument(document, contentToSave)
            .then(() => {
                document.savedContent = contentToSave
                return true
            })
            .catch((error) => {
                onSaveError(error)
                return false
            })
            .finally(() => {
                document.saving = false
                if (savePromises.get(document.path) === savePromise)
                    savePromises.delete(document.path)
            })

        savePromises.set(document.path, savePromise)
        return savePromise
    }

    async function activate(path: string) {
        if (isDeleting(path)) return false
        if (path === activePath.value) return true
        if (!(await saveDocument(activeDocument.value))) return false
        if (isDeleting(path)) return false
        const nextIndex = documents.value.findIndex((document) => document.path === path)
        if (nextIndex === -1) return false
        activeIndex.value = nextIndex
        return true
    }

    async function open(document: StudioDocument, version = pathVersion(document.path)) {
        if (!canOpen(document.path, version)) return false
        const existing = documents.value.find((candidate) => candidate.path === document.path)
        if (existing) return activate(existing.path)
        if (!(await saveDocument(activeDocument.value))) return false
        if (!canOpen(document.path, version)) return false
        documents.value.push(document)
        activeIndex.value = documents.value.length - 1
        return true
    }

    async function close(path: string, reason: 'user' | 'deleted' = 'user') {
        const document = documents.value.find((document) => document.path === path)
        if (!document) return false
        if (reason === 'user' && !(await saveDocument(document))) return false
        if (reason === 'user' && isDeleting(path)) return false
        const index = documents.value.indexOf(document)
        if (index === -1) return false

        const wasActive = activeIndex.value === index
        documents.value.splice(index, 1)
        if (documents.value.length === 0) {
            activeIndex.value = -1
        } else if (wasActive) {
            activeIndex.value = Math.min(index, documents.value.length - 1)
        } else if (index < activeIndex.value) {
            activeIndex.value -= 1
        }
        return true
    }

    async function deletePath(path: string, remove: () => Promise<void>) {
        const session = generation
        const deleting = deletingPaths.value
        if (
            [...deletingPaths.value].some(
                (parent) => containsPath(parent, path) || containsPath(path, parent),
            )
        )
            return false
        deletingPaths.value.add(path)
        try {
            await Promise.all(
                [...savePromises]
                    .filter(([candidate]) => containsPath(path, candidate))
                    .map(([, promise]) => promise),
            )
            await remove()
            if (session !== generation) return true
            pathVersions.set(path, (pathVersions.get(path) ?? 0) + 1)
            for (const document of [...documents.value]) {
                if (containsPath(path, document.path)) await close(document.path, 'deleted')
            }
            return true
        } finally {
            deleting.delete(path)
        }
    }

    function updateActiveContent(content: string) {
        if (activeDocument.value && !isDeleting(activeDocument.value.path))
            activeDocument.value.content = content
    }

    function discardActiveChanges() {
        if (activeDocument.value) activeDocument.value.content = activeDocument.value.savedContent
    }

    async function saveActive() {
        return saveDocument(activeDocument.value)
    }

    async function saveAll() {
        const results = await Promise.all(documents.value.map((document) => saveDocument(document)))
        return results.every(Boolean)
    }

    function reset() {
        generation++
        documents.value = []
        activeIndex.value = -1
        savePromises.clear()
        deletingPaths.value = new Set()
        pathVersions.clear()
    }

    return {
        documents,
        activeDocument,
        activePath,
        hasUnsavedChanges,
        hasAnyUnsavedChanges,
        activate,
        open,
        close,
        deletePath,
        isDeleting,
        pathVersion,
        canOpen,
        saveDocument,
        saveActive,
        saveAll,
        updateActiveContent,
        discardActiveChanges,
        reset,
    }
}
