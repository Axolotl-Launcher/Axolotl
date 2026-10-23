import { useRouter } from 'vue-router'

import { usePreview } from '../../../../../lib/state/index'
import type { OnboardingMode } from './onboarding-config'

/**
 * App.vue's `replayOnboarding`, which the About page injects: the main tour
 * starts again from Home, over whatever page it moves through. The overlay
 * is loaded on demand and handed to the shell's app-overlay host.
 */
export function useReplayOnboarding() {
    const router = useRouter()
    const { ui } = usePreview()

    return async function replayOnboarding(mode: OnboardingMode) {
        const { default: OnboardingOverlay } = await import('./OnboardingOverlay.vue')
        if (mode === 'main') await router.replace('/')
        ui.showAppOverlay({ key: 'onboarding', component: OnboardingOverlay, props: { mode } })
    }
}
