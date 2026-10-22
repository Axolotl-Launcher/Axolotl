import { existsSync, readdirSync, readFileSync, lstatSync } from 'node:fs'
import { dirname, join, relative, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

import YAML from 'yaml'

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..')
const errors = []
const ignoredDirectories = new Set([
    '.git',
    'node_modules',
    'target',
    '.turbo',
    'dist',
    '.nuxt',
    '.output',
])

function walk(directory) {
    if (!existsSync(directory)) return []

    return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
        if (entry.isDirectory() && ignoredDirectories.has(entry.name)) return []

        const path = join(directory, entry.name)
        const relativePath = relative(root, path).replaceAll('\\', '/')
        if (relativePath === '.agents/tmp' || relativePath.startsWith('.agents/tmp/')) return []
        if (relativePath === '.agents/cache' || relativePath.startsWith('.agents/cache/')) return []
        if (entry.isSymbolicLink()) return [path]
        if (entry.isDirectory()) return walk(path)
        return [path]
    })
}

function report(file, message) {
    errors.push(`${relative(root, file)}: ${message}`)
}

const files = walk(root)
if (existsSync(join(root, '.claude'))) {
    errors.push('.claude/: move repository Agent resources into .agents/')
}

const trackedGuidance = files.filter((file) => {
    const path = relative(root, file).replaceAll('\\', '/')
    return (
        path === 'AGENTS.md' ||
        /(^|\/)AGENTS\.md$/.test(path) ||
        path === 'CONTRIBUTING.md' ||
        path.startsWith('.agents/') ||
        path.startsWith('standards/') ||
        path.startsWith('.github/instructions/')
    )
})

for (const file of files) {
    if (file.endsWith('CLAUDE.md')) report(file, 'replace this instruction file with AGENTS.md')
    if (relative(root, file).replaceAll('\\', '/').startsWith('.claude/')) {
        report(file, 'move repository agent resources into .agents/')
    }
}

for (const file of trackedGuidance) {
    if (file.endsWith('.md')) {
        try {
            const stat = lstatSync(file)
            if (
                stat.isSymbolicLink() &&
                !existsSync(resolve(dirname(file), readFileSync(file, 'utf8')))
            ) {
                report(file, 'instruction symlink target does not exist')
            }
            const text = readFileSync(file, 'utf8')
            if (text.includes('.claude/'))
                report(file, 'replace stale .claude paths with .agents paths')

            const links = text.matchAll(/\[[^\]]*\]\(([^)\s]+)(?:\s+[^)]*)?\)/g)
            for (const [, destination] of links) {
                if (/^(?:[a-z][a-z\d+.-]*:|\/|#)/i.test(destination)) continue
                const localPath = destination.split('#', 1)[0].split('?', 1)[0]
                if (
                    localPath &&
                    !existsSync(resolve(dirname(file), decodeURIComponent(localPath)))
                ) {
                    report(file, `broken Markdown link: ${destination}`)
                }
            }
        } catch (error) {
            report(file, `cannot read guidance: ${error.message}`)
        }
    }
}

for (const skillFile of files.filter((file) =>
    /\.agents\/skills\/[^/]+\/SKILL\.md$/.test(file.replaceAll('\\', '/')),
)) {
    const source = readFileSync(skillFile, 'utf8')
    const match = source.match(/^---\r?\n([\s\S]*?)\r?\n---(?:\r?\n|$)/)
    if (!match) {
        report(skillFile, 'missing YAML frontmatter')
        continue
    }

    let metadata
    try {
        metadata = YAML.parse(match[1])
    } catch (error) {
        report(skillFile, `invalid YAML frontmatter: ${error.message}`)
        continue
    }

    const expectedName = relative(dirname(dirname(skillFile)), dirname(skillFile))
    if (metadata?.name !== expectedName)
        report(skillFile, `name must match skill directory (${expectedName})`)
    if (typeof metadata?.description !== 'string' || !metadata.description.trim()) {
        report(skillFile, 'description must be a non-empty string')
    }
}

for (const evalFile of files.filter((file) =>
    file.endsWith('.agents/skills/oiloil-ui-ux-guide/evals/evals.json'),
)) {
    let suite
    try {
        suite = JSON.parse(readFileSync(evalFile, 'utf8'))
    } catch (error) {
        report(evalFile, `invalid JSON: ${error.message}`)
        continue
    }

    if (!Array.isArray(suite.evals) || suite.evals.length === 0)
        report(evalFile, 'evals must be a non-empty array')
    for (const evaluation of suite.evals ?? []) {
        for (const fixture of evaluation.files ?? []) {
            if (!existsSync(resolve(dirname(evalFile), fixture))) {
                report(evalFile, `${evaluation.name}: missing fixture ${fixture}`)
            }
        }
    }
}

if (errors.length) {
    console.error(errors.map((error) => `- ${error}`).join('\n'))
    process.exitCode = 1
} else {
    console.log(`Agent configuration valid (${trackedGuidance.length} guidance files)`)
}
