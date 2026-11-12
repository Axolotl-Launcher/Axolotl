export type FileTreeEntry = {
    relativePath: string
    fileName: string
    id?: string
}

export type FileTreeFolderRow = {
    kind: 'folder'
    path: string
    name: string
    depth: number
    fileCount: number
    expanded: boolean
    childIds: string[]
}

export type FileTreeFileRow<TFile extends FileTreeEntry> = {
    kind: 'file'
    file: TFile
    depth: number
    parentPath: string
}

export type FileTreeRow<TFile extends FileTreeEntry> = FileTreeFolderRow | FileTreeFileRow<TFile>

type FileTreeNode<TFile extends FileTreeEntry> = {
    path: string
    name: string
    directFiles: TFile[]
    children: Map<string, FileTreeNode<TFile>>
    parent?: FileTreeNode<TFile>
    fileCount: number
    fileIds: string[]
    sorted: boolean
}

function createFileTreeNode<TFile extends FileTreeEntry>(
    path: string,
    name: string,
): FileTreeNode<TFile> {
    return {
        path,
        name,
        directFiles: [],
        children: new Map(),
        fileCount: 0,
        fileIds: [],
        sorted: false,
    }
}

function filePathSegments(relativePath: string): string[] {
    return relativePath.split(/[\\/]/).filter(Boolean)
}

function fileParentPath(relativePath: string): string {
    return filePathSegments(relativePath).slice(0, -1).join('/')
}

function buildFileTree<TFile extends FileTreeEntry>(files: readonly TFile[]): FileTreeNode<TFile> {
    const root = createFileTreeNode<TFile>('', '')
    for (const file of files) {
        const segments = filePathSegments(file.relativePath)
        let node = root
        let path = ''
        for (let index = 0; index < segments.length - 1; index += 1) {
            path = path ? `${path}/${segments[index]}` : segments[index]
            let child = node.children.get(path)
            if (!child) {
                child = createFileTreeNode<TFile>(path, segments[index])
                child.parent = node
                node.children.set(path, child)
            }
            node = child
        }
        node.directFiles.push(file)
        for (
            let countNode: FileTreeNode<TFile> | undefined = node;
            countNode;
            countNode = countNode.parent
        ) {
            countNode.fileCount += 1
            if (file.id) countNode.fileIds.push(file.id)
        }
    }
    return root
}

function appendFolderRows<TFile extends FileTreeEntry>(
    node: FileTreeNode<TFile>,
    depth: number,
    expandedFolders: ReadonlySet<string>,
    rows: FileTreeRow<TFile>[],
    compareText: (left: string, right: string) => number,
) {
    if (!node.sorted) {
        node.children = new Map(
            [...node.children.entries()].sort(([, left], [, right]) =>
                compareText(left.name, right.name),
            ),
        )
        node.directFiles.sort((left, right) => compareText(left.fileName, right.fileName))
        node.sorted = true
    }
    const children = [...node.children.values()]
    const files = node.directFiles
    let folderIndex = 0
    let fileIndex = 0
    while (folderIndex < children.length || fileIndex < files.length) {
        const folder = children[folderIndex]
        const file = files[fileIndex]
        if (!folder || (file && compareText(folder.name, file.fileName) > 0)) {
            rows.push({
                kind: 'file',
                file,
                depth,
                parentPath: node.path,
            })
            fileIndex += 1
            continue
        }
        rows.push({
            kind: 'folder',
            path: folder.path,
            name: folder.name,
            depth,
            fileCount: folder.fileCount,
            expanded: expandedFolders.has(folder.path),
            childIds: folder.fileIds,
        })
        if (expandedFolders.has(folder.path)) {
            appendFolderRows(folder, depth + 1, expandedFolders, rows, compareText)
        }
        folderIndex += 1
    }
}

export function collectFileTreeFolders(files: readonly FileTreeEntry[]): string[] {
    const folders = new Set<string>()
    for (const file of files) {
        const segments = filePathSegments(file.relativePath)
        for (let index = 1; index < segments.length; index += 1) {
            folders.add(segments.slice(0, index).join('/'))
        }
    }
    return [...folders].sort()
}

/** Retains the tree and locale ordering while only folder expansion changes. */
export function createFileTreeRowBuilder<TFile extends FileTreeEntry>(
    files: readonly TFile[],
    locale = 'en',
): (expandedFolders: ReadonlySet<string>) => FileTreeRow<TFile>[] {
    const root = buildFileTree(files)
    const compareText = new Intl.Collator(locale, { sensitivity: 'base' }).compare
    return (expandedFolders) => {
        const rows: FileTreeRow<TFile>[] = []
        appendFolderRows(root, 0, expandedFolders, rows, compareText)
        return rows
    }
}

export function buildFileTreeRows<TFile extends FileTreeEntry>(
    files: readonly TFile[],
    expandedFolders: ReadonlySet<string>,
    searchQuery: string,
    locale = 'en',
): FileTreeRow<TFile>[] {
    const query = searchQuery.trim().toLocaleLowerCase(locale)
    if (query) {
        return files
            .filter((file) => file.relativePath.toLocaleLowerCase(locale).includes(query))
            .map((file) => ({
                kind: 'file',
                file,
                depth: 0,
                parentPath: fileParentPath(file.relativePath),
            }))
    }

    return createFileTreeRowBuilder(files, locale)(expandedFolders)
}
