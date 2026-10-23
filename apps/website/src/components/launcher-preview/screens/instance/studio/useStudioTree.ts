import { computed, onScopeDispose, ref } from 'vue'

export interface StudioTreeNode {
    name: string
    path: string
    type: 'directory' | 'file'
    depth: number
    expanded: boolean
    loaded: boolean
    loading: boolean
    children: StudioTreeNode[]
}

export function useStudioTree(
    readDirectory: (path: string) => Promise<{ name: string; isDirectory: boolean }[]>,
    onError: (error: unknown) => void,
) {
    const rootNodes = ref<StudioTreeNode[]>([])
    const treeLoading = ref(true)
    let revision = 0
    let workspace = 0
    let intentRevision = 0
    const directoryState = new Map<string, Pick<StudioTreeNode, 'expanded' | 'loaded'>>()
    let disposed = false
    function reset() {
        revision++
        workspace++
        directoryState.clear()
        rootNodes.value = []
        treeLoading.value = true
    }
    onScopeDispose(() => {
        disposed = true
        revision++
        workspace++
    })

    async function listDirectory(path: string, depth: number): Promise<StudioTreeNode[]> {
        const entries = await readDirectory(path)
        return entries
            .map((entry) => ({
                name: entry.name,
                path: path ? `${path}/${entry.name}` : entry.name,
                type: entry.isDirectory ? ('directory' as const) : ('file' as const),
                depth,
                expanded: false,
                loaded: false,
                loading: false,
                children: [],
            }))
            .sort((a, b) => {
                if (a.type !== b.type) return a.type === 'directory' ? -1 : 1
                return a.name.localeCompare(b.name, undefined, {
                    numeric: true,
                    sensitivity: 'base',
                })
            })
    }

    async function restoreLoadedDirectories(nodes: StudioTreeNode[]) {
        await Promise.all(
            nodes.map(async (node) => {
                if (node.type !== 'directory') return
                const previous = directoryState.get(node.path)
                if (!previous?.loaded) return
                if (!node.loaded) {
                    node.children = await listDirectory(node.path, node.depth + 1)
                    node.loaded = true
                }
                node.expanded = directoryState.get(node.path)?.expanded ?? false
                await restoreLoadedDirectories(node.children)
            }),
        )
    }

    async function refreshTree() {
        const request = ++revision
        try {
            const nextRoot = await listDirectory('', 0)
            let restoredIntent: number
            do {
                restoredIntent = intentRevision
                await restoreLoadedDirectories(nextRoot)
            } while (!disposed && request === revision && restoredIntent !== intentRevision)
            if (!disposed && request === revision) rootNodes.value = nextRoot
        } finally {
            if (!disposed && request === revision) treeLoading.value = false
        }
    }

    function flattenTree(nodes: StudioTreeNode[]): StudioTreeNode[] {
        return nodes.flatMap((node) => [
            node,
            ...(node.type === 'directory' && node.expanded ? flattenTree(node.children) : []),
        ])
    }

    const visibleNodes = computed(() => flattenTree(rootNodes.value))

    async function loadRoot() {
        treeLoading.value = true
        try {
            await refreshTree()
        } catch (error) {
            if (!disposed) onError(error)
        }
    }

    async function toggleDirectory(node: StudioTreeNode) {
        if (node.loading) return
        if (node.loaded) {
            node.expanded = !node.expanded
            directoryState.set(node.path, { loaded: true, expanded: node.expanded })
            intentRevision++
            return
        }

        const request = workspace
        directoryState.set(node.path, { loaded: true, expanded: true })
        intentRevision++
        node.loading = true
        try {
            const children = await listDirectory(node.path, node.depth + 1)
            if (disposed || request !== workspace) return
            const current = visibleNodes.value.find((entry) => entry.path === node.path) ?? node
            current.children = children
            current.loaded = true
            current.expanded = directoryState.get(node.path)?.expanded ?? true
        } catch (error) {
            if (!disposed && request === workspace) {
                directoryState.delete(node.path)
                onError(error)
            }
        } finally {
            node.loading = false
        }
    }

    return {
        rootNodes,
        treeLoading,
        visibleNodes,
        listDirectory,
        loadRoot,
        refreshTree,
        toggleDirectory,
        reset,
    }
}
