import { onBeforeUnmount, ref } from 'vue'

import { usePreview } from '../../../../lib/state/index'
import { useJre } from './jre'

/**
 * `useJavaTest` (apps/app-frontend/src/composables/useJavaTest.ts): tests a Java path
 * with `test_jre` (or `get_jre` without a major version), debounced while typing. The
 * debounce runs on the window's clock and is dropped with the component.
 */
export default function useJavaTest() {
    const { clock } = usePreview()
    const jre = useJre()
    const testingJava = ref(false)
    const javaTestResult = ref<boolean | null>(null)
    let cancelDebounce: (() => void) | null = null

    function clearDebounce() {
        cancelDebounce?.()
        cancelDebounce = null
    }

    async function runJavaTest(path: string, version: number | null, _track = true) {
        clearDebounce()
        if (!path) {
            javaTestResult.value = null
            return
        }
        testingJava.value = true
        try {
            if (version == null) {
                javaTestResult.value = !!(await jre.getJre(path))
            } else {
                javaTestResult.value = await jre.testJre(path, version)
            }
        } catch {
            javaTestResult.value = false
        }
        testingJava.value = false
    }

    function testJavaInstallationDebounced(path: string, version: number | null, delay = 600) {
        clearDebounce()
        if (!path) {
            javaTestResult.value = null
            return
        }
        cancelDebounce = clock.after(delay, () => {
            cancelDebounce = null
            void runJavaTest(path, version, false)
        })
    }

    async function testJavaInstallation(path: string, version: number | null, track = false) {
        await runJavaTest(path, version, track)
    }

    onBeforeUnmount(clearDebounce)

    return {
        testingJava,
        javaTestResult,
        testJavaInstallationDebounced,
        testJavaInstallation,
    }
}
