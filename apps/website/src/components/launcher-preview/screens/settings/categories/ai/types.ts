/*
 * The AI provider types of the desktop app (apps/app-frontend/src/helpers/ai.ts),
 * which mirror the serialized structs of packages/app-lib/src/api/ai.rs.
 */

export type AIProtocol =
    | 'openai'
    | 'anthropic'
    | 'google'
    | 'ollama'
    | 'azure'
    | 'azure-ai'
    | 'bedrock'
    | 'cloudflare'
    | 'huggingface'
    | 'router'

export type AIAuthType = 'apiKey' | 'oauthDeviceFlow' | 'none'

export interface AIProviderDefinition {
    id: string
    name: string
    protocol: AIProtocol
    auth_type: AIAuthType
    default_endpoint: string
    check_model: string
    show_model_fetcher: boolean
    required_settings: string[]
    sponsored: boolean
}

export interface AIProviderModel {
    id: string
    name: string
    enabled: boolean
    source: 'builtin' | 'custom' | 'remote'
}

export interface AIProviderConfig {
    provider_id: string
    custom_name: string | null
    protocol: AIProtocol
    enabled: boolean
    endpoint: string
    settings: Record<string, string>
    has_api_key: boolean
    configured_credentials: string[]
    oauth_connected: boolean
    models: AIProviderModel[]
}

export interface AIState {
    settings: { enabled: boolean }
    catalog_source: string
    providers: AIProviderConfig[]
}

export interface AIProviderConfigUpdate {
    provider_id: string
    custom_name: string | null
    enabled: boolean
    endpoint: string
    settings: Record<string, string>
}

export interface AIModelUpdate {
    provider_id: string
    model_id: string
    display_name: string
    enabled: boolean
}

export interface OAuthDeviceCode {
    flow_id: string
    user_code: string
    verification_uri: string
    verification_uri_complete: string | null
    expires_in: number
    interval: number
}

export type OAuthPollStatus = 'pending' | 'success' | 'expired' | 'denied' | 'slow_down'

/** `AiTextRequest` of ai.rs: what `ai::complete_text` sends to a provider. */
export interface AITextRequest {
    provider_id: string
    model_id: string
    system_prompt: string
    user_prompt: string
    /** `AiTextMode`; translation requests disable reasoning where a model allows it. */
    mode?: 'default' | 'translation'
    /** `AiTextResponseFormat`. */
    response_format?: 'text' | 'json_object'
}

/** What a completion's `respond` callback is given once the request went through. */
export interface AICompletionContext {
    definition: AIProviderDefinition
    config: AIProviderConfig
    modelId: string
}

/**
 * The payload an `invoke` of the AI plugin rejects with: theseus errors are
 * serialized as `{ field_name, message }` (apps/app/src/api/mod.rs), not as
 * an `Error`, so the notification toast shows them the way the app does.
 */
export interface AIInvokeError {
    field_name: 'Theseus'
    message: string
}
