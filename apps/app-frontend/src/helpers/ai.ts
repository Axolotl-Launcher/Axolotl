import { ref } from 'vue'

import { invoke } from '@/helpers/invoke'

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

export const sharedAIState = ref<AIState | null>(null)

export async function getAICatalog(): Promise<AIProviderDefinition[]> {
    return await invoke('plugin:ai|ai_get_catalog')
}

export async function getAIState(): Promise<AIState> {
    const state = await invoke<AIState>('plugin:ai|ai_get_state')
    sharedAIState.value = state
    return state
}

export async function updateAISettings(enabled: boolean): Promise<void> {
    await invoke('plugin:ai|ai_update_settings', { settings: { enabled } })
    if (sharedAIState.value) sharedAIState.value.settings.enabled = enabled
}

export async function updateAIProvider(update: AIProviderConfigUpdate): Promise<void> {
    await invoke('plugin:ai|ai_update_provider', { update })
}

export async function setAIProviderKey(providerId: string, secret: string | null): Promise<void> {
    await invoke('plugin:ai|ai_set_api_key', { providerId, secret })
}

export async function setAIProviderCredential(
    providerId: string,
    credential: string,
    secret: string | null,
): Promise<void> {
    await invoke('plugin:ai|ai_set_credential', { providerId, credential, secret })
}

export async function updateAIModel(update: AIModelUpdate): Promise<void> {
    await invoke('plugin:ai|ai_update_model', { update })
}

export async function removeAIModel(providerId: string, modelId: string): Promise<void> {
    await invoke('plugin:ai|ai_remove_model', { providerId, modelId })
}

export async function fetchAIModels(providerId: string): Promise<AIProviderModel[]> {
    return await invoke('plugin:ai|ai_fetch_models', { providerId })
}

export async function testAIProvider(providerId: string, modelId: string): Promise<string> {
    return await invoke('plugin:ai|ai_test_provider', { providerId, modelId })
}

export async function beginAIOAuth(providerId: string): Promise<OAuthDeviceCode> {
    return await invoke('plugin:ai|ai_begin_oauth', { providerId })
}

export async function pollAIOAuth(flowId: string): Promise<OAuthPollStatus> {
    return await invoke('plugin:ai|ai_poll_oauth', { flowId })
}

export async function disconnectAIOAuth(providerId: string): Promise<void> {
    await invoke('plugin:ai|ai_disconnect_oauth', { providerId })
}
