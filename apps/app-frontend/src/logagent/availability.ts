import type { AnalysisSource } from './types'

// Keep the cloud integration available in code while its entry points are paused.
export const CLOUD_LOGAGENT_ENABLED = false

export function availableAnalysisSource(source: AnalysisSource): AnalysisSource {
    return CLOUD_LOGAGENT_ENABLED ? source : 'custom'
}
