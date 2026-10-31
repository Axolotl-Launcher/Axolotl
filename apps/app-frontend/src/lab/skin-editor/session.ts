export const SKIN_EDITOR_PREFERENCES_KEY = 'axolotl-skin-editor-settings'

export const DEFAULT_SKIN_EDITOR_PREFERENCES: SkinEditorPreferences = {
    autoSaveEnabled: true,
    autoSaveIntervalSeconds: 30,
    warnBeforeLeave: true,
}

export type SkinEditorAutoSaveStatus = 'idle' | 'saving' | 'saved' | 'error'

export interface SkinEditorPreferences {
    autoSaveEnabled: boolean
    autoSaveIntervalSeconds: number
    warnBeforeLeave: boolean
}

export interface SkinEditorRecovery {
    updatedAt: number
    name?: string
}

export type SkinEditorParentMessage =
    | {
          type: 'axolotl-skin-theme'
          theme: {
              dark: boolean
              colors: Record<string, string>
              metrics: Record<string, string>
          }
      }
    | {
          type: 'axolotl-skin-config'
          autoSaveEnabled: boolean
          autoSaveIntervalSeconds: number
      }
    | {
          type: 'axolotl-skin-command'
          requestId: string
          command:
              'flush-draft' | 'discard-draft' | 'restore-draft' | 'start-new' | 'request-export'
          reason?: 'manual' | 'leave'
      }
    | {
          type: 'axolotl-skin-export-result'
          requestId: string
          saved: boolean
      }
    | {
          type: 'axolotl-skin-export-accepted'
          requestId: string
          accepted: boolean
      }

export type SkinEditorEditorMessage =
    | {
          type: 'axolotl-skin-theme-ready'
          recovery?: SkinEditorRecovery
      }
    | {
          type: 'axolotl-skin-state'
          dirty: boolean
          autoSave: SkinEditorAutoSaveStatus
          lastSavedAt?: number
      }
    | {
          type: 'axolotl-skin-export-request'
          requestId: string
          name: string
          dataUrl: string
          reason: 'manual' | 'leave'
      }
    | {
          type: 'axolotl-skin-command-result'
          requestId: string
          ok: boolean
      }
    | { type: 'axolotl-skin-new-skin-request' }

type StorageLike = Pick<Storage, 'getItem' | 'setItem'>

export function clampAutoSaveInterval(value: number): number {
    if (!Number.isFinite(value)) return DEFAULT_SKIN_EDITOR_PREFERENCES.autoSaveIntervalSeconds
    return Math.min(600, Math.max(5, Math.round(value)))
}

export function normalizeSkinEditorPreferences(value: unknown): SkinEditorPreferences {
    if (!value || typeof value !== 'object') return { ...DEFAULT_SKIN_EDITOR_PREFERENCES }

    const candidate = value as Partial<SkinEditorPreferences>
    return {
        autoSaveEnabled:
            typeof candidate.autoSaveEnabled === 'boolean'
                ? candidate.autoSaveEnabled
                : DEFAULT_SKIN_EDITOR_PREFERENCES.autoSaveEnabled,
        autoSaveIntervalSeconds:
            typeof candidate.autoSaveIntervalSeconds === 'number'
                ? clampAutoSaveInterval(candidate.autoSaveIntervalSeconds)
                : DEFAULT_SKIN_EDITOR_PREFERENCES.autoSaveIntervalSeconds,
        warnBeforeLeave:
            typeof candidate.warnBeforeLeave === 'boolean'
                ? candidate.warnBeforeLeave
                : DEFAULT_SKIN_EDITOR_PREFERENCES.warnBeforeLeave,
    }
}

export function loadSkinEditorPreferences(
    storage: StorageLike | undefined = globalThis.localStorage,
): SkinEditorPreferences {
    try {
        return normalizeSkinEditorPreferences(
            JSON.parse(storage?.getItem(SKIN_EDITOR_PREFERENCES_KEY) ?? 'null'),
        )
    } catch {
        return { ...DEFAULT_SKIN_EDITOR_PREFERENCES }
    }
}

export function saveSkinEditorPreferences(
    preferences: SkinEditorPreferences,
    storage: StorageLike | undefined = globalThis.localStorage,
) {
    const normalized = normalizeSkinEditorPreferences(preferences)
    if (!normalized.autoSaveEnabled) normalized.warnBeforeLeave = true
    storage?.setItem(SKIN_EDITOR_PREFERENCES_KEY, JSON.stringify(normalized))
    return normalized
}

export function shouldPromptBeforeLeave(
    preferences: SkinEditorPreferences,
    dirty: boolean,
): boolean {
    if (!dirty) return false
    return !preferences.autoSaveEnabled || preferences.warnBeforeLeave
}

export function shouldKeepRecoveryDraftAfterExport(
    exportedRevision: number,
    currentRevision: number,
): boolean {
    return exportedRevision !== currentRevision
}

export function normalizeExportFilename(name: string): string {
    const trimmed = name.trim()
    if (!trimmed) return 'skin.png'
    return trimmed.toLowerCase().endsWith('.png') ? trimmed : `${trimmed}.png`
}

export function createSkinEditorRequestId(): string {
    if (typeof crypto !== 'undefined' && 'randomUUID' in crypto) return crypto.randomUUID()
    return `${Date.now()}-${Math.random().toString(36).slice(2)}`
}

function hasPngSignature(dataUrl: string): boolean {
    const encoded = dataUrl.slice('data:image/png;base64,'.length)
    if (!encoded || !/^[A-Za-z0-9+/]+={0,2}$/.test(encoded)) return false
    try {
        const signature = atob(encoded.slice(0, 16))
        return (
            signature.charCodeAt(0) === 0x89 &&
            signature.charCodeAt(1) === 0x50 &&
            signature.charCodeAt(2) === 0x4e &&
            signature.charCodeAt(3) === 0x47 &&
            signature.charCodeAt(4) === 0x0d &&
            signature.charCodeAt(5) === 0x0a &&
            signature.charCodeAt(6) === 0x1a &&
            signature.charCodeAt(7) === 0x0a
        )
    } catch {
        return false
    }
}

export function parseSkinEditorMessage(data: unknown): SkinEditorEditorMessage | null {
    if (!data || typeof data !== 'object') return null
    const message = data as Record<string, unknown>

    if (message.type === 'axolotl-skin-theme-ready') {
        if (message.recovery === undefined) return { type: message.type }
        if (!message.recovery || typeof message.recovery !== 'object') return null
        const recovery = message.recovery as Record<string, unknown>
        if (typeof recovery.updatedAt !== 'number') return null
        if (recovery.name !== undefined && typeof recovery.name !== 'string') return null
        return {
            type: message.type,
            recovery: {
                updatedAt: recovery.updatedAt,
                ...(typeof recovery.name === 'string' ? { name: recovery.name } : {}),
            },
        }
    }

    if (message.type === 'axolotl-skin-state') {
        if (
            typeof message.dirty !== 'boolean' ||
            !['idle', 'saving', 'saved', 'error'].includes(String(message.autoSave))
        ) {
            return null
        }
        if (message.lastSavedAt !== undefined && typeof message.lastSavedAt !== 'number') {
            return null
        }
        return {
            type: message.type,
            dirty: message.dirty,
            autoSave: message.autoSave as SkinEditorAutoSaveStatus,
            ...(typeof message.lastSavedAt === 'number'
                ? { lastSavedAt: message.lastSavedAt }
                : {}),
        }
    }

    if (message.type === 'axolotl-skin-export-request') {
        const dataUrl = typeof message.dataUrl === 'string' ? message.dataUrl : ''
        if (
            typeof message.requestId !== 'string' ||
            typeof message.name !== 'string' ||
            !dataUrl.startsWith('data:image/png;base64,') ||
            dataUrl.length > 16 * 1024 * 1024 ||
            !hasPngSignature(dataUrl) ||
            (message.reason !== 'manual' && message.reason !== 'leave')
        ) {
            return null
        }
        return {
            type: message.type,
            requestId: message.requestId,
            name: message.name,
            dataUrl,
            reason: message.reason,
        }
    }

    if (message.type === 'axolotl-skin-command-result') {
        if (typeof message.requestId !== 'string' || typeof message.ok !== 'boolean') return null
        return { type: message.type, requestId: message.requestId, ok: message.ok }
    }

    if (message.type === 'axolotl-skin-new-skin-request') return { type: message.type }

    return null
}
