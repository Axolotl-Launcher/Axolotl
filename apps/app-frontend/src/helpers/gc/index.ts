export {
    buildGcCandidateChain,
    getResolvedStrategyName,
    resolveAutoGcStrategy,
} from './auto-selector'
export { collectGcContext } from './context'
export { createGcPresets } from './gc-presets'
export { detectGcStrategy, GC_STRATEGY_DEFINITIONS, getStrategyBaseArgs } from './strategies.ts'
export type {
    GcContext,
    GcResolution,
    GcStrategyDefinition,
    GcStrategyId,
    ResolvedGcStrategyId,
} from './types'
