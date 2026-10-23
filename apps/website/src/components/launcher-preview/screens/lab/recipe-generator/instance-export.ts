import { usePreview } from '../../../lib/state'
import { useInstanceFiles } from '../../instance/files/instance-files'
import { createDatapackBlob, type PackFile } from './datapack.ts'

function normalizePackFileName(fileName: string): string {
    const segments = fileName.replaceAll('\\', '/').split('/').filter(Boolean)
    const safeName = segments[segments.length - 1] ?? fileName
    return safeName.toLowerCase().endsWith('.zip') ? safeName : `${safeName}.zip`
}

/**
 * Port of the app's `exportDatapackToWorld`
 * (`instance_install_datapack_to_world_bytes`): the generated datapack is
 * written to the world's `datapacks` folder in the instance's folder, where the
 * instance's Files tab shows it. Call from a component's setup.
 */
export function useDatapackExport() {
    const preview = usePreview()
    const { treeFor } = useInstanceFiles()

    async function exportDatapackToWorld(
        instanceId: string,
        worldPath: string,
        files: PackFile[],
        fileName = 'axolotl-recipes.zip',
    ): Promise<string> {
        const instance = preview.instances.get(instanceId)
        if (!instance) throw new Error(`Instance not found: ${instanceId}`)
        await preview.downloads.ensureContents()
        const blob = createDatapackBlob(files)
        const name = normalizePackFileName(fileName)
        const segments = [
            'saves',
            ...worldPath.replaceAll('\\', '/').split('/').filter(Boolean),
            'datapacks',
        ]
        let folder = treeFor(instance)
        const now = Date.now()
        for (const segment of segments) {
            folder.children ??= []
            let next = folder.children.find((child) => child.name === segment)
            if (!next) {
                next = { kind: 'dir', name: segment, modified: now, created: now, children: [] }
                folder.children.push(next)
            }
            if (next.kind !== 'dir') throw new Error(`Not a directory (os error 20): ${segment}`)
            folder = next
        }
        folder.children ??= []
        folder.children = folder.children.filter((child) => child.name !== name)
        folder.children.push({ kind: 'file', name, size: blob.size, modified: now, created: now })
        folder.modified = now
        return [...segments, name].join('/')
    }

    return { exportDatapackToWorld }
}
