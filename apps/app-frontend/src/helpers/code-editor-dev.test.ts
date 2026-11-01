import { execFile } from 'node:child_process'
import { mkdir, mkdtemp, readFile, rm } from 'node:fs/promises'
import { execPath } from 'node:process'
import { fileURLToPath, pathToFileURL } from 'node:url'

import { loadConfigFromFile, optimizeDeps, resolveConfig } from 'vite'
import { expect, it } from 'vitest'

it('the desktop dev optimizer exposes the lazy code editor as ESM', async () => {
    const appRoot = fileURLToPath(new URL('../../', import.meta.url))
    const evidenceRoot = fileURLToPath(new URL('../../../../.agents/plans/', import.meta.url))
    await mkdir(evidenceRoot, { recursive: true })
    const cacheDir = await mkdtemp(`${evidenceRoot}/code-editor-dev-`)

    try {
        const loaded = await loadConfigFromFile(
            { command: 'serve', mode: 'test' },
            `${appRoot}/vite.config.ts`,
        )
        expect(loaded).not.toBeNull()
        const config = await resolveConfig(
            {
                ...loaded!.config,
                root: appRoot,
                configFile: false,
                cacheDir,
                // UI/devtools plugins need a server lifecycle; dependency
                // conversion itself only needs the app's optimizer settings.
                plugins: [],
            },
            'serve',
        )
        // Exercise the real dev configuration without scanning unrelated app
        // entries, listening on a port, or touching an active dev server cache.
        config.optimizeDeps.entries = []
        config.optimizeDeps.include = config.optimizeDeps.include?.filter((entry) =>
            /(?:^| > )(?:ace-builds|vue3-ace-editor)$/.test(entry),
        )
        const metadata = await optimizeDeps(config, true)
        const ace = metadata.optimized['@modrinth/ui > ace-builds']
        const editor = metadata.optimized['@modrinth/ui > vue3-ace-editor']
        expect(ace).toBeDefined()
        expect(editor).toBeDefined()
        expect(ace.needsInterop).toBe(true)

        // Import the actual optimized artifact: the raw Ace entry has no ESM
        // default export and cannot satisfy vue3-ace-editor's default import.
        // A native ESM import, rather than Vitest's module runner, also checks
        // the export contract consumed by the browser. Ace expects window.
        await new Promise<void>((resolve, reject) =>
            execFile(
                execPath,
                [
                    '--input-type=module',
                    '-e',
                    `globalThis.window = globalThis;
const { default: ace } = await import(process.argv[1]);
if (typeof ace.edit !== 'function' || typeof ace.define !== 'function') {
    throw new Error('Optimized Ace has no default editor API');
}`,
                    pathToFileURL(ace.file).href,
                ],
                (error) => (error ? reject(error) : resolve()),
            ),
        )
        expect(await readFile(editor.file, 'utf8')).toContain('VAceEditor')
    } finally {
        await rm(cacheDir, { recursive: true, force: true })
    }
})
