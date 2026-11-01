#!/usr/bin/env node
/**
 * Creates an empty lab plugin so a new one starts from the real contract
 * instead of a copy of another plugin that may drift.
 *
 *   node scripts/axolotl/new-lab-plugin.mjs <id> ["Display name"]
 *
 * The folder name becomes the manifest id and the route `/lab/p/<id>`, so the
 * id is validated here rather than at build time.
 */
import fs from 'node:fs/promises'
import path from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'

const SCRIPT_DIR = path.dirname(fileURLToPath(import.meta.url))
const REPOSITORY_ROOT = path.resolve(SCRIPT_DIR, '..', '..')
const PLUGIN_ROOT = path.join(REPOSITORY_ROOT, 'apps/app-frontend/src/lab/plugins/packages')

const PLUGIN_ID_PATTERN = /^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$/

/** `motd-preview` becomes `MotdPreview`, used for file and component names. */
export function toPascalCase(id) {
    return id
        .split('-')
        .filter(Boolean)
        .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
        .join('')
}

/** Files to write for a new plugin, keyed by file name inside the plugin folder. */
export function scaffoldFiles({ id, name }) {
    const componentName = `${toPascalCase(id)}Tool`
    const index = `import { defineMessage } from '@modrinth/ui'

import { defineLabPlugin } from '@/lab/plugins/types'

import ${componentName} from './${componentName}.vue'

const name = defineMessage({
    id: 'app.lab.plugins.${id}.name',
    defaultMessage: ${JSON.stringify(name)},
})

const description = defineMessage({
    id: 'app.lab.plugins.${id}.description',
    defaultMessage: 'Describe what this plugin does.',
})

export default defineLabPlugin({
    manifest: {
        id: '${id}',
        name,
        description,
        version: '1.0.0',
        category: 'creation',
    },
    tool: {
        title: name,
        description,
        component: ${componentName},
    },
})
`

    const component = `<script setup lang="ts">
import { defineMessages, useVIntl } from '@modrinth/ui'
import { ref } from 'vue'

import { useLabPluginContext } from '@/lab/plugins/types'

const { formatMessage } = useVIntl()
const context = useLabPluginContext()

const messages = defineMessages({
    counter: { id: 'app.lab.plugins.${id}.counter', defaultMessage: 'Clicks' },
    add: { id: 'app.lab.plugins.${id}.add', defaultMessage: 'Add one' },
})

// Plugin storage is namespaced by plugin id, so this key cannot collide with
// another plugin's.
const count = ref(context.storage.read('count', 0))

function addOne() {
    count.value += 1
    context.storage.write('count', count.value)
}
</script>

<template>
    <section
        class="flex flex-col items-start gap-3 rounded-[var(--radius-lg)] border border-surface-5 bg-surface-2 p-4"
    >
        <p class="m-0 text-sm text-[var(--color-text-tertiary)]">
            {{ formatMessage(messages.counter) }}: {{ count }}
        </p>
        <button
            type="button"
            class="rounded-[var(--radius-md)] bg-brand px-3 py-2 text-sm font-bold text-white"
            @click="addOne"
        >
            {{ formatMessage(messages.add) }}
        </button>
    </section>
</template>
`

    return { index, component, componentName }
}

/**
 * Writes the plugin folder. Refuses to overwrite an existing plugin so a
 * mistyped id cannot destroy work.
 */
export async function createLabPlugin({ root = REPOSITORY_ROOT, id, name }) {
    if (!PLUGIN_ID_PATTERN.test(id)) {
        throw new Error(`"${id}" is not a kebab-case id, for example "my-plugin"`)
    }

    const displayName = name ?? id
    const folder = path.join(root, 'apps/app-frontend/src/lab/plugins/packages', id)
    if (await exists(folder)) {
        throw new Error(`${path.relative(root, folder)} already exists`)
    }

    const { index, component, componentName } = scaffoldFiles({ id, name: displayName })
    await fs.mkdir(folder, { recursive: true })
    await fs.writeFile(path.join(folder, 'index.ts'), index, 'utf8')
    await fs.writeFile(path.join(folder, `${componentName}.vue`), component, 'utf8')

    return {
        folder,
        localeIds: [
            `app.lab.plugins.${id}.name`,
            `app.lab.plugins.${id}.description`,
            `app.lab.plugins.${id}.counter`,
            `app.lab.plugins.${id}.add`,
        ],
    }
}

async function exists(target) {
    try {
        await fs.access(target)
        return true
    } catch {
        return false
    }
}

async function main() {
    const [id, name] = process.argv.slice(2)
    if (!id) {
        process.stderr.write('Usage: node scripts/axolotl/new-lab-plugin.mjs <id> ["Display name"]\n')
        process.exitCode = 1
        return
    }

    const { folder, localeIds } = await createLabPlugin({ id, name })
    process.stdout.write(`Created ${path.relative(REPOSITORY_ROOT, folder)}\n\n`)
    process.stdout.write('Next:\n')
    process.stdout.write('  1. Add these ids to en-US and zh-CN locales:\n')
    for (const localeId of localeIds) process.stdout.write(`       ${localeId}\n`)
    process.stdout.write('  2. Run pnpm app:dev and open Lab, then the plugin card.\n')
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? '').href) {
    main().catch((error) => {
        process.stderr.write(`${error.message}\n`)
        process.exitCode = 1
    })
}
