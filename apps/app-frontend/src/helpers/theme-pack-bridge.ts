import { invoke } from '@tauri-apps/api/core'

import type { ThemePackManifest, ThemePackPermission } from '@/helpers/theme-packs'

/**
 * The page bridge is a deliberately small, fixed method table. Theme pack
 * pages run in a sandboxed iframe without Tauri IPC, so the launcher window is
 * the only component that can reach a command; keeping the table closed means
 * a page cannot ask for anything the manifest did not declare.
 */

export interface ThemePackBridgeRequest {
    type: 'axolotl-theme-request'
    id: string
    method: string
    params?: Record<string, unknown>
}

/** `allow-scripts` only: no same-origin access and no Tauri IPC in pack pages. */
export const THEME_PAGE_SANDBOX = 'allow-scripts'

export interface ThemePackBridgeResponse {
    type: 'axolotl-theme-response'
    id: string
    ok: boolean
    data?: unknown
    error?: string
}

const STORAGE_KEY = /^[A-Za-z0-9._-]{1,128}$/
const MAX_STORAGE_BYTES = 2 * 1024 * 1024
const MAX_REQUEST_ID = 128

function requireRecord(value: unknown): Record<string, unknown> {
    if (!value || typeof value !== 'object' || Array.isArray(value)) {
        throw new Error('Expected an object')
    }
    return value as Record<string, unknown>
}

function requireString(value: unknown, name: string, max = 4096): string {
    if (typeof value !== 'string' || !value || value.length > max) {
        throw new Error(`Expected ${name} to be a non-empty string`)
    }
    return value
}

function requireStorageKey(value: unknown): string {
    const key = requireString(value, 'key', 128)
    if (!STORAGE_KEY.test(key) || key.startsWith('.')) {
        throw new Error('The storage key is invalid')
    }
    return key
}

function requireStringMap(value: unknown): Record<string, string> {
    if (value === undefined) return {}
    const record = requireRecord(value)
    const result: Record<string, string> = {}
    for (const [key, entry] of Object.entries(record)) {
        if (typeof entry !== 'string') {
            throw new Error('Headers must be strings')
        }
        result[key] = entry
    }
    return result
}

type BridgeHandler = (packId: string, params: Record<string, unknown>) => Promise<unknown>

interface BridgeMethod {
    permission: ThemePackPermission
    handler: BridgeHandler
}

const METHODS: Record<string, BridgeMethod> = {
    'network.fetch': {
        permission: 'network',
        handler: (packId, params) =>
            invoke('theme_pack_fetch', {
                packId,
                request: {
                    method: requireString(params.method, 'method', 8),
                    url: requireString(params.url, 'url', 4096),
                    headers: requireStringMap(params.headers),
                    body:
                        params.body === undefined
                            ? null
                            : requireString(params.body, 'body', 4 * 1024 * 1024),
                },
            }),
    },
    'storage.list': {
        permission: 'storage',
        handler: (packId) => invoke('theme_pack_storage_list', { packId }),
    },
    'storage.read': {
        permission: 'storage',
        handler: (packId, params) =>
            invoke('theme_pack_storage_read', {
                packId,
                key: requireStorageKey(params.key),
            }),
    },
    'storage.write': {
        permission: 'storage',
        handler: (packId, params) => {
            const contents = requireString(params.contents, 'contents', MAX_STORAGE_BYTES)
            if (new TextEncoder().encode(contents).length > MAX_STORAGE_BYTES) {
                throw new Error('The storage entry is too large')
            }
            return invoke('theme_pack_storage_write', {
                packId,
                key: requireStorageKey(params.key),
                contents,
            })
        },
    },
    'storage.delete': {
        permission: 'storage',
        handler: (packId, params) =>
            invoke('theme_pack_storage_delete', {
                packId,
                key: requireStorageKey(params.key),
            }),
    },
    'instances.list': {
        permission: 'instances.read',
        handler: (packId) => invoke('theme_pack_instance_list', { packId }),
    },
    'instances.mods': {
        permission: 'instances.read',
        handler: (packId, params) =>
            invoke('theme_pack_instance_mods', {
                packId,
                instanceId: requireString(params.instanceId, 'instanceId', 128),
            }),
    },
    'instances.logs': {
        permission: 'instances.logs',
        handler: (packId, params) =>
            invoke('theme_pack_instance_logs', {
                packId,
                instanceId: requireString(params.instanceId, 'instanceId', 128),
            }),
    },
}

export function parseThemePackRequest(value: unknown): ThemePackBridgeRequest | null {
    if (!value || typeof value !== 'object') return null
    const record = value as Record<string, unknown>
    if (record.type !== 'axolotl-theme-request') return null
    if (typeof record.id !== 'string' || !record.id) return null
    if (record.id.length > MAX_REQUEST_ID) return null
    if (typeof record.method !== 'string' || !record.method) return null
    const params = record.params === undefined ? undefined : record.params
    if (params !== undefined) {
        try {
            requireRecord(params)
        } catch {
            return null
        }
    }
    return {
        type: 'axolotl-theme-request',
        id: record.id,
        method: record.method,
        params: params as Record<string, unknown> | undefined,
    }
}

export async function dispatchThemePackRequest(
    packId: string,
    manifest: ThemePackManifest,
    request: ThemePackBridgeRequest,
): Promise<ThemePackBridgeResponse> {
    const method = METHODS[request.method]
    if (!method) {
        return {
            type: 'axolotl-theme-response',
            id: request.id,
            ok: false,
            error: `The method ${request.method} is not available`,
        }
    }
    if (!manifest.permissions.includes(method.permission)) {
        return {
            type: 'axolotl-theme-response',
            id: request.id,
            ok: false,
            error: `The theme pack does not declare the ${method.permission} permission`,
        }
    }
    try {
        const data = await method.handler(packId, request.params ?? {})
        return { type: 'axolotl-theme-response', id: request.id, ok: true, data }
    } catch (error) {
        return {
            type: 'axolotl-theme-response',
            id: request.id,
            ok: false,
            error: error instanceof Error ? error.message : String(error),
        }
    }
}
