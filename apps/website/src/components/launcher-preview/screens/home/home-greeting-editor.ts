import type { ComputedRef, Ref } from 'vue'

import type { HomeGreetingOverride } from './home-dashboard'

/**
 * String key matches the `accountsCard` convention in this app.
 */
export const HOME_GREETING_EDITOR_KEY = 'homeGreetingEditor'

/**
 * Provided by the home page, which owns the dashboard config, so the greeting
 * heading can be edited from both the dashboard and the minimal layout.
 */
export type HomeGreetingEditorContext = {
    /** Key the text is saved under: the active account id, or the global fallback. */
    accountId: ComputedRef<string>
    /** Name shown when no nickname is saved; null when the account has none. */
    accountName: ComputedRef<string | null>
    override: ComputedRef<HomeGreetingOverride | null>
    /** False when the dashboard has no greeting widget left to save the text into. */
    canEdit: ComputedRef<boolean>
    /** A nickname replaces an account name, so it needs one to replace. */
    canEditNickname: ComputedRef<boolean>
    /** True while the editor is open, so the heading can drop its hover affordance. */
    editing: Ref<boolean>
    open: () => void
}
