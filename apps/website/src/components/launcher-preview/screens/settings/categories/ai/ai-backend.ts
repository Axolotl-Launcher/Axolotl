import { markRaw, reactive, type Ref, toRef } from 'vue'

import { type PreviewState, usePreviewDomain } from '../../../../lib/state/index'
import type {
    AICompletionContext,
    AIInvokeError,
    AIModelUpdate,
    AIProviderConfig,
    AIProviderConfigUpdate,
    AIProviderDefinition,
    AIProviderModel,
    AIState,
    AITextRequest,
    OAuthDeviceCode,
    OAuthPollStatus,
} from './types'

export type {
    AIAuthType,
    AICompletionContext,
    AIInvokeError,
    AIModelUpdate,
    AIProtocol,
    AIProviderConfig,
    AIProviderConfigUpdate,
    AIProviderDefinition,
    AIProviderModel,
    AIState,
    AITextRequest,
    OAuthDeviceCode,
    OAuthPollStatus,
} from './types'

/*
 * The AI plugin of the desktop app (apps/app/src/api/ai.rs over
 * packages/app-lib/src/api/ai.rs) behind the helpers of
 * apps/app-frontend/src/helpers/ai.ts, with the same names and signatures.
 *
 * The SQLite tables (ai_settings, ai_provider_configs, ai_provider_models),
 * the OS credential store and the pending OAuth device flows are kept in
 * memory for the life of the window: secrets a visitor types never leave
 * it. Provider traffic is simulated: requests to loopback or private hosts
 * (Ollama, LM Studio, vLLM ... with nothing listening) fail to connect, public
 * endpoints answer, model discovery returns the provider's catalog of the
 * bundled LobeHub model bank, and OAuth authorization completes in the
 * "browser" a few seconds after it is opened.
 */

export const AI_DOMAIN_KEY = 'settings:ai'

const API_KEY_CREDENTIAL = 'api-key'
const AWS_ACCESS_KEY_ID_CREDENTIAL = 'aws-access-key-id'
const AWS_SECRET_ACCESS_KEY_CREDENTIAL = 'aws-secret-access-key'
const AWS_SESSION_TOKEN_CREDENTIAL = 'aws-session-token'
const VERTEX_SERVICE_ACCOUNT_CREDENTIAL = 'vertex-service-account'

/** The device-flow providers of `oauth_definition` with what their device endpoints hand out. */
const OAUTH_PROVIDERS: Record<
    string,
    { interval: number; expiresIn: number; verificationUri: string }
> = {
    githubcopilot: {
        interval: 5,
        expiresIn: 900,
        verificationUri: 'https://github.com/login/device',
    },
    // The backend's own device-code TTL and page for ChatGPT.
    chatgpt: {
        interval: 8,
        expiresIn: 15 * 60,
        verificationUri: 'https://auth.openai.com/codex/device',
    },
    supergrok: { interval: 5, expiresIn: 900, verificationUri: 'https://auth.x.ai' },
}

/** How long after the authorization page opens the visitor's approval lands. */
const OAUTH_APPROVAL_MS = 7000
const OAUTH_APPROVAL_ON_REOPEN_MS = 1500

/** `test_provider`'s fixed prompt pair. */
const TEST_SYSTEM_PROMPT = 'Reply with a short plain-text greeting.'
const TEST_USER_PROMPT = 'Hello from Axolotl Launcher'

/** What models answer the test prompt with. */
const TEST_GREETINGS = [
    'Hello! Nice to meet you.',
    'Hi there! How can I help you today?',
    'Hello! 👋 Hope your day is going well.',
    'Hey there! Great to hear from you.',
    'Hello, Axolotl Launcher! Ready when you are.',
    'Hi! Wishing you a wonderful day.',
]

/** `is_text_model`: ids of discovered models that are not text-generation models. */
const NON_TEXT_MODEL_MARKERS = [
    'embedding',
    'embed-',
    '-embed',
    '/embed',
    'rerank',
    'dall-e',
    'gpt-image',
    'imagen',
    'image-generation',
    'stable-diffusion',
    'recraft',
    'flux-',
    'sdxl',
    'whisper',
    'transcri',
    'speech',
    'tts',
    'audio',
    'realtime',
    'moderation',
    'video',
    'sora',
    'veo-',
]

interface ProviderRow {
    custom_name: string | null
    protocol: AIProviderDefinition['protocol']
    enabled: boolean
    endpoint: string
    settings: Record<string, string>
}

interface ModelRow {
    display_name: string
    enabled: boolean
    source: AIProviderModel['source']
}

interface OAuthFlow {
    provider_id: string
    /** Epoch milliseconds. */
    expires_at: number
    approved_at: number
}

/** The persisted side: the database rows and the credential store. */
interface AIDatabase {
    enabled: boolean
    configs: Record<string, ProviderRow>
    models: Record<string, Record<string, ModelRow>>
    /** provider id -> credential name -> secret (memory only). */
    credentials: Record<string, Record<string, string>>
    /** Providers with a connected OAuth account. */
    oauth: Record<string, boolean>
    flows: Record<string, OAuthFlow>
}

interface Catalog {
    providers: AIProviderDefinition[]
    source: string
}

type BuiltinModels = Record<string, Array<[string, string, 0 | 1]>>

export interface AIBackend {
    /** `sharedAIState`: the state the last `getAIState()` returned, null before the first call. */
    sharedAIState: Ref<AIState | null>
    getAICatalog: () => Promise<AIProviderDefinition[]>
    getAIState: () => Promise<AIState>
    updateAISettings: (enabled: boolean) => Promise<void>
    updateAIProvider: (update: AIProviderConfigUpdate) => Promise<void>
    setAIProviderKey: (providerId: string, secret: string | null) => Promise<void>
    setAIProviderCredential: (
        providerId: string,
        credential: string,
        secret: string | null,
    ) => Promise<void>
    updateAIModel: (update: AIModelUpdate) => Promise<void>
    removeAIModel: (providerId: string, modelId: string) => Promise<void>
    fetchAIModels: (providerId: string) => Promise<AIProviderModel[]>
    testAIProvider: (providerId: string, modelId: string) => Promise<string>
    /**
     * `ai::complete_text`: the same checks in the same order, then the provider
     * round-trip; resolves with what `respond` produces for the request.
     */
    completeText: (
        request: AITextRequest,
        respond: (context: AICompletionContext) => string | Promise<string>,
    ) => Promise<string>
    beginAIOAuth: (providerId: string) => Promise<OAuthDeviceCode>
    pollAIOAuth: (flowId: string) => Promise<OAuthPollStatus>
    disconnectAIOAuth: (providerId: string) => Promise<void>
    /**
     * Stands in for the system browser the app opens the authorization page
     * in: the visitor approves there shortly after it (re)opens.
     */
    openAuthorizationPage: (flowId: string) => void
}

export interface AIDomainValue {
    /** The app's `sharedAIState` value (null until something called `getAIState()`). */
    state: AIState | null
    db: AIDatabase
    catalog: Catalog | null
    builtin: BuiltinModels | null
    api: AIBackend
}

/** A rejection as `invoke` delivers it. */
function invokeError(message: string): AIInvokeError {
    return { field_name: 'Theseus', message }
}

/** `ErrorKind::InputError`. */
function inputError(message: string): AIInvokeError {
    return invokeError(`Invalid input: ${message}`)
}

/** `ErrorKind::OtherError`. */
function otherError(message: string): AIInvokeError {
    return invokeError(`Error: ${message}`)
}

function hash(value: string): number {
    let result = 0
    for (let index = 0; index < value.length; index++) {
        result = (result * 31 + value.charCodeAt(index)) >>> 0
    }
    return result
}

/** A stable duration per key, so one provider always answers at the same pace. */
function latency(key: string, base: number, spread: number): number {
    return base + (hash(key) % (spread + 1))
}

function compare(left: string, right: string): number {
    return left < right ? -1 : left > right ? 1 : 0
}

/** A copy of `record` without `key` (a row deleted from its table). */
function omit<T>(record: Record<string, T>, key: string): Record<string, T> {
    return Object.fromEntries(Object.entries(record).filter(([entry]) => entry !== key))
}

function credentialNames(providerId: string): string[] {
    switch (providerId) {
        case 'bedrock':
            return [
                API_KEY_CREDENTIAL,
                AWS_ACCESS_KEY_ID_CREDENTIAL,
                AWS_SECRET_ACCESS_KEY_CREDENTIAL,
                AWS_SESSION_TOKEN_CREDENTIAL,
            ]
        case 'vertexai':
            return [VERTEX_SERVICE_ACCOUNT_CREDENTIAL]
        default:
            return [API_KEY_CREDENTIAL]
    }
}

/** `normalize_endpoint`. */
function normalizeEndpoint(endpoint: string, fallback: string): string {
    const value = endpoint.trim() || fallback.trim()
    if (!value) return ''
    let url: URL
    try {
        url = new URL(value)
    } catch {
        throw inputError('AI provider endpoint must be a valid URL')
    }
    if (url.protocol !== 'http:' && url.protocol !== 'https:') {
        throw inputError('AI provider endpoint must use HTTP or HTTPS')
    }
    return value.replace(/\/+$/, '')
}

/** `endpoint_with_path`. */
function endpointWithPath(base: string, path: string): string {
    return base.endsWith(path) ? base : `${base.replace(/\/+$/, '')}${path}`
}

/** Hosts the visitor's Mac has nothing listening on: loopback, private ranges, `.local`. */
function isLocalHost(hostname: string): boolean {
    const host = hostname.replace(/^\[|\]$/g, '').toLowerCase()
    return (
        host === 'localhost' ||
        host.endsWith('.localhost') ||
        host.endsWith('.local') ||
        host === '::1' ||
        host === '0.0.0.0' ||
        /^127\./.test(host) ||
        /^10\./.test(host) ||
        /^192\.168\./.test(host) ||
        /^172\.(1[6-9]|2\d|3[01])\./.test(host)
    )
}

/** `github_copilot_uses_responses`. */
function githubCopilotUsesResponses(model: string): boolean {
    const id = model.toLowerCase()
    if (
        id.startsWith('oswe-') ||
        [
            'o1-pro',
            'o1-pro-2025-03-19',
            'o3-deep-research',
            'o3-deep-research-2025-06-26',
            'o3-pro',
            'o3-pro-2025-06-10',
            'o4-mini-deep-research',
            'o4-mini-deep-research-2025-06-26',
            'codex-mini-latest',
            'computer-use-preview',
            'computer-use-preview-2025-03-11',
            'gpt-5-mini',
            'gpt-5-mini-2025-08-07',
        ].includes(id)
    ) {
        return true
    }
    if (!id.startsWith('gpt-5') || id.includes('-chat')) return false
    if (id.includes('-codex') || id.includes('-pro')) return true
    if (!id.startsWith('gpt-5.')) return false
    const minor = id.slice('gpt-5.'.length).split(/[-:]/)[0] ?? ''
    return /^\d+$/.test(minor) && Number(minor) >= 2
}

/** What `EncodingKey::from_rsa_pem` accepts, as far as a PEM block's shape goes. */
function isRsaPrivateKeyPem(value: string): boolean {
    const match = value
        .trim()
        .match(/^-----BEGIN (RSA )?PRIVATE KEY-----([\s\S]+?)-----END (RSA )?PRIVATE KEY-----$/)
    if (!match || match[1] !== match[3]) return false
    const body = match[2].replace(/\s+/g, '')
    return body.length >= 256 && /^[A-Za-z0-9+/]+={0,2}$/.test(body)
}

function randomUserCode(): string {
    const alphabet = 'BCDFGHJKLMNPQRSTVWXZ'
    let code = ''
    for (let index = 0; index < 8; index++) {
        if (index === 4) code += '-'
        code += alphabet[Math.floor(Math.random() * alphabet.length)]
    }
    return code
}

function randomUuid(): string {
    if (typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function') {
        return crypto.randomUUID()
    }
    return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, (char) => {
        const value = Math.floor(Math.random() * 16)
        return (char === 'x' ? value : (value & 0x3) | 0x8).toString(16)
    })
}

function createApi(domain: AIDomainValue, preview: PreviewState): AIBackend {
    const wait = (ms: number) =>
        new Promise<void>((resolve) => {
            preview.clock.after(ms, resolve)
        })

    async function catalogData(): Promise<Catalog> {
        if (!domain.catalog) {
            const module = await import('./data/provider-catalog')
            domain.catalog = markRaw({ providers: module.PROVIDERS, source: module.CATALOG_SOURCE })
        }
        return domain.catalog
    }

    async function builtinData(): Promise<BuiltinModels> {
        if (!domain.builtin) {
            const module = await import('./data/builtin-models')
            domain.builtin = markRaw(module.BUILTIN_MODELS)
        }
        return domain.builtin
    }

    async function loaded() {
        const [catalog, builtin] = await Promise.all([catalogData(), builtinData()])
        return { catalog, builtin }
    }

    /** `provider_definition`. */
    function definitionOf(catalog: Catalog, providerId: string): AIProviderDefinition {
        const definition = catalog.providers.find((provider) => provider.id === providerId)
        if (!definition) throw inputError(`Unknown AI provider: ${providerId}`)
        return definition
    }

    function builtinModels(builtin: BuiltinModels, providerId: string) {
        return (builtin[providerId] ?? []).map(([id, name, enabled]) => ({
            id,
            name: name || id,
            enabled: enabled === 1,
        }))
    }

    /** `load_provider_models`: the bundled models with saved overrides, then the rest by name. */
    function providerModels(builtin: BuiltinModels, providerId: string): AIProviderModel[] {
        const stored = new Map(Object.entries(domain.db.models[providerId] ?? {}))
        const models: AIProviderModel[] = builtinModels(builtin, providerId).map((model) => {
            const saved = stored.get(model.id)
            stored.delete(model.id)
            return {
                id: model.id,
                name: saved?.display_name || model.name,
                enabled: saved ? saved.enabled : model.enabled,
                source: 'builtin',
            }
        })
        const additional = [...stored].map(([id, row]) => ({
            id,
            name: row.display_name,
            enabled: row.enabled,
            source: row.source,
        }))
        additional.sort(
            (left, right) => compare(left.name, right.name) || compare(left.id, right.id),
        )
        return [...models, ...additional]
    }

    /** `load_provider_config`. */
    function providerConfig(
        builtin: BuiltinModels,
        definition: AIProviderDefinition,
    ): AIProviderConfig {
        const row = domain.db.configs[definition.id]
        const models = providerModels(builtin, definition.id)
        if (!models.length && definition.check_model) {
            models.push({
                id: definition.check_model,
                name: definition.check_model,
                enabled: true,
                source: 'builtin',
            })
        }
        const credentials = domain.db.credentials[definition.id] ?? {}
        return {
            provider_id: definition.id,
            custom_name: row ? row.custom_name : null,
            protocol: row ? row.protocol : definition.protocol,
            enabled: row ? row.enabled : false,
            endpoint: row
                ? normalizeEndpoint(row.endpoint, definition.default_endpoint)
                : definition.default_endpoint,
            settings: row ? { ...row.settings } : {},
            has_api_key: !!credentials[API_KEY_CREDENTIAL],
            configured_credentials: credentialNames(definition.id).filter(
                (name) => !!credentials[name],
            ),
            oauth_connected: !!domain.db.oauth[definition.id],
            models,
        }
    }

    /** The HTTP round-trip: unreachable hosts fail to connect, others answer after `ms`. */
    async function send(providerName: string, url: string, ms: number) {
        let href: string
        let hostname: string
        try {
            const parsed = new URL(url)
            href = parsed.href
            hostname = parsed.hostname
        } catch {
            await wait(8)
            throw otherError(`AI_NETWORK_FAILED: Could not reach ${providerName}: builder error`)
        }
        if (isLocalHost(hostname)) {
            await wait(latency(href, 12, 24))
            throw otherError(
                `AI_NETWORK_FAILED: Could not reach ${providerName}: error sending request for url (${href})`,
            )
        }
        await wait(ms)
    }

    /** The request URL of a completion, after the credential checks of each protocol. */
    async function completionUrl(
        definition: AIProviderDefinition,
        config: AIProviderConfig,
        request: AITextRequest,
    ): Promise<string> {
        const credentials = domain.db.credentials[definition.id] ?? {}
        const missingCredentials = () =>
            inputError(`Configure credentials for ${definition.name} before using it`)
        const model = request.model_id
        switch (config.protocol) {
            case 'anthropic':
                if (!credentials[API_KEY_CREDENTIAL]) throw missingCredentials()
                return endpointWithPath(config.endpoint, '/v1/messages')
            case 'google': {
                if (definition.id === 'vertexai') return vertexUrl(config, credentials, model)
                const apiKey = credentials[API_KEY_CREDENTIAL]
                if (!apiKey) throw missingCredentials()
                return `${config.endpoint.replace(/\/+$/, '')}/v1beta/models/${encodeURIComponent(model)}:generateContent?key=${encodeURIComponent(apiKey)}`
            }
            case 'bedrock': {
                const region = config.settings.region?.trim() ? config.settings.region : 'us-east-1'
                const base = config.endpoint || `https://bedrock-runtime.${region}.amazonaws.com`
                const authMode = config.settings.auth_mode
                const useApiKey =
                    authMode === 'api-key'
                        ? true
                        : authMode === 'aws-credentials'
                          ? false
                          : !!credentials[API_KEY_CREDENTIAL]
                if (useApiKey) {
                    if (!credentials[API_KEY_CREDENTIAL]) {
                        throw inputError('Configure the Bedrock API key')
                    }
                } else {
                    if (!credentials[AWS_ACCESS_KEY_ID_CREDENTIAL]) {
                        throw inputError(
                            'Configure a Bedrock API key or AWS credentials before using it',
                        )
                    }
                    if (!credentials[AWS_SECRET_ACCESS_KEY_CREDENTIAL]) {
                        throw inputError('Configure the Bedrock AWS secret access key')
                    }
                }
                return `${base.replace(/\/+$/, '')}/model/${encodeURIComponent(model)}/invoke`
            }
            case 'ollama':
                return endpointWithPath(config.endpoint, '/api/chat')
            case 'cloudflare': {
                if (!credentials[API_KEY_CREDENTIAL]) {
                    throw inputError('Configure a Cloudflare API token before using it')
                }
                const accountId = config.settings.account_id
                if (!accountId) {
                    throw inputError('Configure the Cloudflare account ID before using it')
                }
                return `${config.endpoint.replace(/\/+$/, '')}/accounts/${encodeURIComponent(accountId)}/ai/run/${model}`
            }
            default: {
                if (definition.auth_type === 'oauthDeviceFlow') {
                    if (!domain.db.oauth[definition.id]) {
                        throw inputError(`Connect ${definition.id} with OAuth before using it`)
                    }
                } else if (definition.auth_type !== 'none' && !credentials[API_KEY_CREDENTIAL]) {
                    throw missingCredentials()
                }
                if (definition.id === 'githubcopilot' && model.toLowerCase().includes('claude')) {
                    return endpointWithPath(config.endpoint, '/v1/messages')
                }
                if (definition.id === 'chatgpt') {
                    return endpointWithPath(config.endpoint, '/responses')
                }
                if (definition.id === 'githubcopilot' && githubCopilotUsesResponses(model)) {
                    return endpointWithPath(config.endpoint, '/responses')
                }
                if (config.protocol === 'azure') {
                    const deployment = config.settings.deployment || model
                    const version = config.settings.api_version ?? '2024-10-21'
                    return `${config.endpoint.replace(/\/+$/, '')}/openai/deployments/${encodeURIComponent(deployment)}/chat/completions?api-version=${encodeURIComponent(version)}`
                }
                return endpointWithPath(config.endpoint, '/chat/completions')
            }
        }
    }

    /** `complete_vertex` up to the request: the service account, its token, the model URL. */
    async function vertexUrl(
        config: AIProviderConfig,
        credentials: Record<string, string>,
        model: string,
    ): Promise<string> {
        const serviceAccount = credentials[VERTEX_SERVICE_ACCOUNT_CREDENTIAL]
        if (!serviceAccount) {
            throw inputError('Configure Vertex AI service-account credentials before using it')
        }
        let account: Record<string, unknown>
        try {
            account = JSON.parse(serviceAccount)
        } catch {
            account = {}
        }
        const field = (name: string) => account?.[name]
        if (
            !account ||
            typeof account !== 'object' ||
            Array.isArray(account) ||
            typeof field('client_email') !== 'string' ||
            typeof field('private_key') !== 'string' ||
            typeof field('project_id') !== 'string' ||
            (field('token_uri') !== undefined && typeof field('token_uri') !== 'string')
        ) {
            throw inputError('Vertex AI credentials must be a service-account JSON document')
        }
        const clientEmail = field('client_email') as string
        const privateKey = field('private_key') as string
        const projectId = field('project_id') as string
        if (!clientEmail.trim() || !privateKey.trim() || !projectId.trim()) {
            throw inputError('Vertex AI service-account credentials are incomplete')
        }
        const tokenUri =
            (field('token_uri') as string | undefined) ?? 'https://oauth2.googleapis.com/token'
        let tokenUrl: URL | null = null
        try {
            tokenUrl = new URL(tokenUri)
        } catch {
            tokenUrl = null
        }
        if (!tokenUrl || tokenUrl.protocol !== 'https:') {
            throw inputError('Vertex AI token URI must be a valid HTTPS URL')
        }
        if (!isRsaPrivateKeyPem(privateKey)) throw inputError('Vertex AI private key is invalid')
        await send('Vertex AI OAuth', tokenUrl.href, latency(tokenUrl.href, 180, 140))

        const project = config.settings.project?.trim() ? config.settings.project : projectId
        const locationSetting =
            'location' in config.settings ? config.settings.location : config.settings.region
        const location = locationSetting?.trim() ? locationSetting : 'global'
        const base =
            config.endpoint ||
            (location === 'global'
                ? 'https://aiplatform.googleapis.com'
                : `https://${location}-aiplatform.googleapis.com`)
        return `${base.replace(/\/+$/, '')}/v1/projects/${encodeURIComponent(project)}/locations/${encodeURIComponent(location)}/publishers/google/models/${encodeURIComponent(model)}:generateContent`
    }

    async function getAICatalog(): Promise<AIProviderDefinition[]> {
        const catalog = await catalogData()
        await wait(latency('catalog', 8, 10))
        return catalog.providers.map((definition) => ({
            ...definition,
            required_settings: [...definition.required_settings],
        }))
    }

    async function getAIState(): Promise<AIState> {
        const { catalog, builtin } = await loaded()
        await wait(latency('state', 28, 20))
        domain.state = {
            settings: { enabled: domain.db.enabled },
            catalog_source: catalog.source,
            providers: catalog.providers.map((definition) => providerConfig(builtin, definition)),
        }
        return domain.state
    }

    async function updateAISettings(enabled: boolean): Promise<void> {
        await wait(latency('settings', 24, 20))
        domain.db.enabled = enabled
        if (domain.state) domain.state.settings.enabled = enabled
    }

    async function updateAIProvider(update: AIProviderConfigUpdate): Promise<void> {
        const catalog = await catalogData()
        await wait(latency(`save:${update.provider_id}`, 30, 30))
        const definition = definitionOf(catalog, update.provider_id)
        const endpoint = normalizeEndpoint(update.endpoint, definition.default_endpoint)
        if (
            update.enabled &&
            !endpoint &&
            definition.protocol !== 'bedrock' &&
            definition.id !== 'vertexai'
        ) {
            throw inputError('Configure the provider endpoint before enabling it')
        }
        domain.db.configs[definition.id] = {
            custom_name: update.custom_name === null ? null : update.custom_name.trim(),
            protocol: definition.protocol,
            enabled: update.enabled,
            endpoint,
            settings: { ...update.settings },
        }
    }

    async function setAIProviderCredential(
        providerId: string,
        credential: string,
        secret: string | null,
    ): Promise<void> {
        const catalog = await catalogData()
        await wait(latency(`keychain:${providerId}:${credential}`, 36, 40))
        const definition = definitionOf(catalog, providerId)
        if (definition.auth_type === 'oauthDeviceFlow') {
            throw inputError('This provider uses OAuth authorization instead of stored credentials')
        }
        if (!credentialNames(providerId).includes(credential)) {
            throw inputError(`Unsupported credential for ${definition.name}`)
        }
        // A blank entry reads as no credential, as `read_credential` filters them.
        const stored = (domain.db.credentials[providerId] ??= {})
        stored[credential] = secret?.trim() ?? ''
    }

    async function setAIProviderKey(providerId: string, secret: string | null): Promise<void> {
        await setAIProviderCredential(providerId, API_KEY_CREDENTIAL, secret)
    }

    async function updateAIModel(update: AIModelUpdate): Promise<void> {
        const { catalog, builtin } = await loaded()
        await wait(latency(`model:${update.provider_id}`, 20, 24))
        definitionOf(catalog, update.provider_id)
        const modelId = update.model_id.trim()
        if (!modelId) throw inputError('AI model ID cannot be empty')
        const displayName = update.display_name.trim() || modelId
        const source = (builtin[update.provider_id] ?? []).some(([id]) => id === modelId)
            ? 'builtin'
            : 'custom'
        const rows = (domain.db.models[update.provider_id] ??= {})
        const existing = rows[modelId]
        rows[modelId] = {
            display_name: displayName,
            enabled: update.enabled,
            source: existing && source !== 'builtin' ? existing.source : source,
        }
    }

    async function removeAIModel(providerId: string, modelId: string): Promise<void> {
        const catalog = await catalogData()
        await wait(latency(`model:${providerId}`, 20, 24))
        definitionOf(catalog, providerId)
        const rows = domain.db.models[providerId]
        if (rows) domain.db.models[providerId] = omit(rows, modelId)
    }

    async function fetchAIModels(providerId: string): Promise<AIProviderModel[]> {
        const { catalog, builtin } = await loaded()
        await wait(latency('db', 12, 12))
        const definition = definitionOf(catalog, providerId)
        if (!definition.show_model_fetcher) {
            throw inputError(`${definition.name} does not expose model discovery`)
        }
        const config = providerConfig(builtin, definition)
        let key = ''
        if (definition.auth_type === 'oauthDeviceFlow') {
            if (!domain.db.oauth[definition.id]) {
                throw inputError(`Connect ${definition.id} with OAuth before using it`)
            }
            key = 'oauth'
        } else {
            key = domain.db.credentials[definition.id]?.[API_KEY_CREDENTIAL] ?? ''
        }
        let endpoint: string
        switch (config.protocol) {
            case 'google':
                endpoint = `${config.endpoint}/v1beta/models?key=${encodeURIComponent(key)}`
                break
            case 'ollama':
                endpoint = endpointWithPath(config.endpoint, '/api/tags')
                break
            case 'anthropic':
                endpoint = endpointWithPath(config.endpoint, '/v1/models')
                break
            case 'cloudflare': {
                const accountId = config.settings.account_id?.trim()
                if (!accountId) {
                    throw inputError('Configure the Cloudflare account ID before fetching models')
                }
                endpoint = `${config.endpoint.replace(/\/+$/, '')}/accounts/${encodeURIComponent(accountId)}/ai/models/search`
                break
            }
            default:
                endpoint = endpointWithPath(config.endpoint, '/models')
        }
        await send(definition.name, endpoint, latency(`models:${definition.id}`, 520, 640))
        if (!key && definition.auth_type === 'apiKey') {
            throw otherError(
                `AI_AUTHENTICATION_FAILED: ${definition.name} returned HTTP 401 Unauthorized: {"error":{"message":"Unauthorized"}}`,
            )
        }

        // The provider lists the models of its catalog; OpenAI-style `/models`
        // responses carry no display names, Anthropic and Google ones do.
        const named = config.protocol === 'anthropic' || config.protocol === 'google'
        let discovered = builtinModels(builtin, definition.id).map(
            (model) => [model.id, named ? model.name : model.id] as const,
        )
        if (!discovered.length && definition.check_model) {
            discovered = [[definition.check_model, definition.check_model]]
        }
        discovered = discovered
            .filter(([id]) => {
                const lower = id.toLowerCase()
                return !NON_TEXT_MODEL_MARKERS.some((marker) => lower.includes(marker))
            })
            .sort((left, right) => compare(left[0], right[0]))
            .filter((entry, index, list) => index === 0 || list[index - 1][0] !== entry[0])
        if (!discovered.length) {
            throw otherError(`${definition.name} returned no text models from ${endpoint}`)
        }
        const rows = (domain.db.models[definition.id] ??= {})
        for (const [id, name] of discovered) {
            const existing = rows[id]
            if (existing) {
                if (existing.source !== 'custom') existing.display_name = name
            } else {
                rows[id] = { display_name: name, enabled: true, source: 'remote' }
            }
        }
        return providerModels(builtin, definition.id)
    }

    async function completeText(
        request: AITextRequest,
        respond: (context: AICompletionContext) => string | Promise<string>,
    ): Promise<string> {
        const { catalog, builtin } = await loaded()
        await wait(latency('db', 12, 12))
        if (!domain.db.enabled) throw inputError('AI features are disabled')
        const definition = definitionOf(catalog, request.provider_id)
        const config = providerConfig(builtin, definition)
        if (!config.enabled) throw inputError(`${definition.name} is disabled`)
        if (!request.model_id.trim()) throw inputError('Select an AI model before using it')
        const url = await completionUrl(definition, config, request)
        const thinking = request.mode === 'translation' ? 0 : 1
        await send(
            definition.name,
            url,
            latency(`complete:${definition.id}:${request.model_id}`, 780, 900) + thinking * 260,
        )
        return await respond({ definition, config, modelId: request.model_id })
    }

    async function testAIProvider(providerId: string, modelId: string): Promise<string> {
        return completeText(
            {
                provider_id: providerId,
                model_id: modelId,
                system_prompt: TEST_SYSTEM_PROMPT,
                user_prompt: TEST_USER_PROMPT,
                mode: 'default',
                response_format: 'text',
            },
            ({ definition, modelId: model }) =>
                TEST_GREETINGS[hash(`${definition.id}:${model}`) % TEST_GREETINGS.length],
        )
    }

    async function beginAIOAuth(providerId: string): Promise<OAuthDeviceCode> {
        const definition = OAUTH_PROVIDERS[providerId]
        if (!definition) {
            await wait(10)
            throw inputError(`${providerId} does not support OAuth device authorization`)
        }
        await wait(latency(`oauth:${providerId}`, 420, 260))
        const flowId = randomUuid()
        const now = Date.now()
        domain.db.flows[flowId] = {
            provider_id: providerId,
            expires_at: now + definition.expiresIn * 1000,
            approved_at: now + OAUTH_APPROVAL_MS,
        }
        return {
            flow_id: flowId,
            user_code: randomUserCode(),
            verification_uri: definition.verificationUri,
            verification_uri_complete: null,
            expires_in: definition.expiresIn,
            interval: definition.interval,
        }
    }

    async function pollAIOAuth(flowId: string): Promise<OAuthPollStatus> {
        await wait(latency(`poll:${flowId}`, 240, 180))
        const flow = domain.db.flows[flowId]
        if (!flow) throw inputError('OAuth flow was not found or already finished')
        const now = Date.now()
        if (flow.expires_at <= now) {
            domain.db.flows = omit(domain.db.flows, flowId)
            return 'expired'
        }
        if (flow.approved_at > now) return 'pending'
        domain.db.oauth[flow.provider_id] = true
        domain.db.flows = omit(domain.db.flows, flowId)
        return 'success'
    }

    async function disconnectAIOAuth(providerId: string): Promise<void> {
        await wait(latency(`keychain:${providerId}:oauth`, 30, 30))
        if (!OAUTH_PROVIDERS[providerId]) {
            throw inputError(`${providerId} does not support OAuth device authorization`)
        }
        domain.db.oauth[providerId] = false
    }

    function openAuthorizationPage(flowId: string) {
        const flow = domain.db.flows[flowId]
        if (!flow) return
        flow.approved_at = Math.min(flow.approved_at, Date.now() + OAUTH_APPROVAL_ON_REOPEN_MS)
    }

    // The bundled catalog is read on the first call anyway; fetch its chunks now
    // so the first page that asks does not wait for them (a failed load is
    // retried by that call).
    loaded().catch(() => {})

    return {
        sharedAIState: toRef(domain, 'state'),
        getAICatalog,
        getAIState,
        updateAISettings,
        updateAIProvider,
        setAIProviderKey,
        setAIProviderCredential,
        updateAIModel,
        removeAIModel,
        fetchAIModels,
        testAIProvider,
        completeText,
        beginAIOAuth,
        pollAIOAuth,
        disconnectAIOAuth,
        openAuthorizationPage,
    }
}

/**
 * The AI backend of the window. Call it in `setup`; every caller shares one
 * domain, created on first use with the app's fresh-install state (AI
 * features on, every provider off).
 */
export function useAIBackend(): AIBackend {
    return usePreviewDomain<AIDomainValue>(AI_DOMAIN_KEY, (preview) => {
        const value = reactive({
            state: null,
            db: { enabled: true, configs: {}, models: {}, credentials: {}, oauth: {}, flows: {} },
            catalog: null,
            builtin: null,
            api: null,
        }) as unknown as AIDomainValue
        value.api = markRaw(createApi(value, preview))
        return { value }
    }).api
}
