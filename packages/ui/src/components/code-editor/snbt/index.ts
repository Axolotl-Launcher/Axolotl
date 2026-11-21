import type { Monaco } from '../types.ts'
import { registerSnbtCompletions } from './completions.ts'
import { registerSnbtLanguage } from './language.ts'

export { attachSnbtCompletion, registerSnbtCompletions } from './completions.ts'
export { registerSnbtLanguage, SNBT_LANGUAGE_ID } from './language.ts'
export { scanSnbt } from './scan.ts'
export type {
    SnbtCandidate,
    SnbtCandidateKind,
    SnbtCompletionHandle,
    SnbtCompletionSource,
    SnbtExpect,
    SnbtFrame,
    SnbtScan,
    SnbtSprite,
} from './types.ts'

/** 一次注册 SNBT 的全部语言能力：词法、括号、缩进、补全。幂等。 */
export function registerSnbt(monaco: Monaco): void {
    registerSnbtLanguage(monaco)
    registerSnbtCompletions(monaco)
}
