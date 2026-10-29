import { execFileSync } from 'node:child_process'
import fs from 'node:fs/promises'
import { existsSync, readFileSync, realpathSync, statSync } from 'node:fs'
import { stripTypeScriptTypes } from 'node:module'
import { dirname, extname, join, relative, resolve, sep } from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'
import vm from 'node:vm'

const REPOSITORY_ROOT = fileURLToPath(new URL('../../', import.meta.url))
const CATALOG_PATH = resolve(REPOSITORY_ROOT, 'packages/app-lib/src/api/lobehub_text_models.json')
const RUST_API_PATH = resolve(REPOSITORY_ROOT, 'packages/app-lib/src/api/ai.rs')
const SOURCE_PATTERN = /^LobeHub ([0-9a-f]{40}) model-bank chat models$/
const RUST_SOURCE_PATTERN = /const CATALOG_SOURCE: &str = "LobeHub ([0-9a-f]{40})";/g

// Sponsored providers are paid partner listings that must survive every
// synchronization, even if a future rewrite of ai.rs drops upstream providers.
// Keep this list in sync with SPONSORED_PROVIDERS in packages/app-lib/src/api/ai.rs.
const SPONSORED_PROVIDER_IDS = ['codeflow', 'fastai']

function assertSponsoredProvidersPreserved(rustSource) {
    for (const providerId of SPONSORED_PROVIDER_IDS) {
        const pattern = new RegExp(`provider!\\s*\\(\\s*"${providerId}"`)
        if (!pattern.test(rustSource)) {
            throw new Error(
                `Sponsored provider ${providerId} is missing from ai.rs; refusing to rewrite it`,
            )
        }
    }
}

function usage() {
    console.log(`Usage: node --experimental-vm-modules scripts/axolotl/sync-lobehub-models.mjs \\
    --upstream <lobehub checkout> [--commit <40-character SHA>] [--check]`)
}

function parseArguments(arguments_) {
    const options = { check: false, commit: undefined, upstream: undefined }
    for (let index = 0; index < arguments_.length; index++) {
        const argument = arguments_[index]
        switch (argument) {
            case '--check':
                options.check = true
                break
            case '--commit':
            case '--upstream': {
                const value = arguments_[++index]
                if (!value) throw new Error(`${argument} requires a value`)
                options[argument.slice(2)] = value
                break
            }
            case '--help':
            case '-h':
                usage()
                process.exit(0)
            default:
                throw new Error(`Unknown argument: ${argument}`)
        }
    }
    if (!options.upstream) throw new Error('--upstream is required')
    return options
}

function requireObject(value, label) {
    if (!value || typeof value !== 'object' || Array.isArray(value)) {
        throw new Error(`${label} must be an object`)
    }
    return value
}

function countModels(providers) {
    return Object.values(providers).reduce((total, models) => total + models.length, 0)
}

function resolveCommit(upstreamRoot, requestedCommit) {
    const commit =
        requestedCommit ??
        execFileSync('git', ['-C', upstreamRoot, 'rev-parse', 'HEAD'], {
            encoding: 'utf8',
        }).trim()
    if (!/^[0-9a-f]{40}$/.test(commit)) {
        throw new Error(`Invalid LobeHub commit SHA: ${commit}`)
    }
    return commit
}

function createModelBankLoader(upstreamRoot) {
    if (typeof vm.SourceTextModule !== 'function') {
        throw new Error('Node.js must be run with --experimental-vm-modules')
    }

    const sourceRoot = realpathSync(resolve(upstreamRoot, 'packages/model-bank/src'))
    const context = vm.createContext(Object.create(null), {
        codeGeneration: { strings: false, wasm: false },
        name: 'lobehub-model-bank',
    })
    const moduleCache = new Map()
    const schema = vm.runInContext(
        `(() => {
            let schema
            schema = new Proxy(function () { return schema }, { get() { return schema } })
            return schema
        })()`,
        context,
    )
    const zodModule = new vm.SyntheticModule(
        ['z'],
        function () {
            this.setExport('z', schema)
        },
        { context, identifier: 'sandbox:zod' },
    )
    const typeFestModule = new vm.SyntheticModule([], function () {}, {
        context,
        identifier: 'sandbox:type-fest',
    })

    function resolveLocalImport(specifier, parentPath) {
        const basePath = resolve(dirname(parentPath), specifier)
        const candidates = extname(basePath)
            ? [basePath]
            : [`${basePath}.ts`, join(basePath, 'index.ts')]
        for (const candidate of candidates) {
            if (!existsSync(candidate) || !statSync(candidate).isFile()) continue
            const canonicalPath = realpathSync(candidate)
            const relativePath = relative(sourceRoot, canonicalPath)
            if (relativePath === '..' || relativePath.startsWith(`..${sep}`)) {
                throw new Error(`Import escapes model-bank/src: ${specifier}`)
            }
            return canonicalPath
        }
        throw new Error(`Unable to resolve ${specifier} from ${parentPath}`)
    }

    function getModule(filePath) {
        const cached = moduleCache.get(filePath)
        if (cached) return cached
        const code = stripTypeScriptTypes(readFileSync(filePath, 'utf8'), {
            mode: 'transform',
            sourceUrl: pathToFileURL(filePath).href,
        })
        const module = new vm.SourceTextModule(code, {
            context,
            identifier: pathToFileURL(filePath).href,
            importModuleDynamically() {
                throw new Error('Dynamic imports are not allowed in the LobeHub model bank')
            },
        })
        moduleCache.set(filePath, module)
        return module
    }

    function linker(specifier, referencingModule) {
        if (specifier === 'zod') return zodModule
        if (specifier === 'type-fest') return typeFestModule
        if (!specifier.startsWith('.')) {
            throw new Error(`External import is not allowed in the model bank: ${specifier}`)
        }
        return getModule(resolveLocalImport(specifier, fileURLToPath(referencingModule.identifier)))
    }

    return async function loadModels() {
        const entryPath = realpathSync(join(sourceRoot, 'aiModels/index.ts'))
        const entry = getModule(entryPath)
        await entry.link(linker)
        await entry.evaluate({ timeout: 20_000 })
        return entry.namespace.LOBE_DEFAULT_MODEL_LIST
    }
}

function validateCurrentCatalog(catalog, rustSource) {
    requireObject(catalog, 'Current model catalog')
    const providers = requireObject(catalog.providers, 'Current model catalog providers')
    const sourceMatch = SOURCE_PATTERN.exec(catalog.source)
    if (!sourceMatch) throw new Error('Current model catalog has an invalid source')

    const rustMatches = [...rustSource.matchAll(RUST_SOURCE_PATTERN)]
    if (rustMatches.length !== 1) {
        throw new Error('Expected exactly one CATALOG_SOURCE constant in ai.rs')
    }
    if (rustMatches[0][1] !== sourceMatch[1]) {
        throw new Error('The JSON and Rust catalog source commits do not match')
    }

    for (const [providerId, models] of Object.entries(providers)) {
        if (!/^[a-z0-9]+$/.test(providerId) || !Array.isArray(models)) {
            throw new Error(`Invalid current catalog provider: ${providerId}`)
        }
    }
    return providers
}

function buildCatalog(models, currentProviders) {
    if (!Array.isArray(models)) {
        throw new Error('LOBE_DEFAULT_MODEL_LIST must be an array')
    }

    const providers = Object.fromEntries(
        Object.keys(currentProviders).map((providerId) => [providerId, []]),
    )
    const seen = new Set()
    let upstreamChatCount = 0

    for (const model of models) {
        requireObject(model, 'LobeHub model')
        if (model.type !== 'chat') continue
        upstreamChatCount++
        if (typeof model.providerId !== 'string' || !model.providerId) {
            throw new Error('LobeHub chat model has no providerId')
        }
        if (typeof model.id !== 'string' || !model.id.trim()) {
            throw new Error(`LobeHub ${model.providerId} chat model has no id`)
        }
        if (typeof model.enabled !== 'boolean') {
            throw new Error(`LobeHub model ${model.providerId}/${model.id} has invalid enabled state`)
        }
        if (!Object.hasOwn(providers, model.providerId)) continue

        const key = `${model.providerId}\0${model.id}`
        if (seen.has(key)) throw new Error(`Duplicate LobeHub model: ${model.providerId}/${model.id}`)
        seen.add(key)
        const displayName = model.displayName || model.id
        if (typeof displayName !== 'string' || !displayName.trim()) {
            throw new Error(`LobeHub model ${model.providerId}/${model.id} has no display name`)
        }
        providers[model.providerId].push({
            id: model.id,
            name: displayName,
            enabled: model.enabled,
        })
    }

    for (const providerId of SPONSORED_PROVIDER_IDS) {
        const sponsorModels = currentProviders[providerId]
        if (!sponsorModels?.length) continue
        providers[providerId] = [...sponsorModels]
        console.log(
            `Preserved ${sponsorModels.length} sponsored model(s) for ${providerId}`,
        )
    }

    const previousCount = countModels(currentProviders)
    const synchronizedCount = countModels(providers)
    const minimumCount = Math.max(100, Math.floor(previousCount * 0.7))
    const maximumCount = Math.max(500, Math.ceil(previousCount * 2))
    if (synchronizedCount < minimumCount || synchronizedCount > maximumCount) {
        throw new Error(
            `Refusing suspicious model count change: ${previousCount} -> ${synchronizedCount}`,
        )
    }
    for (const [providerId, previousModels] of Object.entries(currentProviders)) {
        if (previousModels.length > 0 && providers[providerId].length === 0) {
            throw new Error(`Refusing to empty existing provider ${providerId}`)
        }
    }

    return { providers, synchronizedCount, upstreamChatCount }
}

const options = parseArguments(process.argv.slice(2))
const upstreamRoot = realpathSync(resolve(options.upstream))
const commit = resolveCommit(upstreamRoot, options.commit)
const [catalogText, rustSource] = await Promise.all([
    fs.readFile(CATALOG_PATH, 'utf8'),
    fs.readFile(RUST_API_PATH, 'utf8'),
])
assertSponsoredProvidersPreserved(rustSource)
const currentCatalog = JSON.parse(catalogText)
const currentProviders = validateCurrentCatalog(currentCatalog, rustSource)
const loadModels = createModelBankLoader(upstreamRoot)
const { providers, synchronizedCount, upstreamChatCount } = buildCatalog(
    await loadModels(),
    currentProviders,
)
const providersChanged = JSON.stringify(providers) !== JSON.stringify(currentProviders)

if (!providersChanged) {
    console.log(
        `LobeHub ${commit} has no model changes for ${Object.keys(providers).length} supported providers (${synchronizedCount}/${upstreamChatCount} chat models).`,
    )
    process.exit(0)
}

const nextCatalogText = `${JSON.stringify(
    {
        source: `LobeHub ${commit} model-bank chat models`,
        providers,
    },
    null,
    2,
)}\n`
const nextRustSource = rustSource.replace(
    RUST_SOURCE_PATTERN,
    `const CATALOG_SOURCE: &str = "LobeHub ${commit}";`,
)
assertSponsoredProvidersPreserved(nextRustSource)

if (options.check) {
    console.error(
        `Bundled model catalog is stale: LobeHub ${commit} has ${synchronizedCount}/${upstreamChatCount} applicable chat models.`,
    )
    process.exit(1)
}

await Promise.all([
    fs.writeFile(CATALOG_PATH, nextCatalogText),
    fs.writeFile(RUST_API_PATH, nextRustSource),
])
console.log(
    `Synchronized ${synchronizedCount}/${upstreamChatCount} LobeHub chat models from ${commit} across ${Object.keys(providers).length} supported providers.`,
)
