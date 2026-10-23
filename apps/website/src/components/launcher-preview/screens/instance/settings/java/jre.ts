import { ref } from 'vue'

import { usePreview } from '../../../../lib/state'
import type { JavaVersion, SimulatedMachine } from './machine'

/*
 * `helpers/jre.js` and `composables/useJavaTest.ts` over the simulated machine:
 * a path is a working Java when it is one of the machine's runtimes.
 */

export function useJre(machine: SimulatedMachine) {
    const preview = usePreview()

    function wait(ms: number) {
        return new Promise<void>((resolve) => {
            preview.clock.after(ms, resolve)
        })
    }

    /** `get_jre(path)`: the runtime at `path`, or an error when it is not a Java executable. */
    async function getJre(path: string): Promise<JavaVersion> {
        await wait(420)
        const java = machine.javas.find((entry) => entry.path === path.trim())
        if (!java) throw new Error(`Not a valid Java executable: ${path}`)
        return { ...java }
    }

    /** `test_jre(path, majorVersion)`. */
    async function testJre(path: string, majorVersion: number): Promise<boolean> {
        const java = await getJre(path).catch(() => null)
        return !!java && java.parsed_version === majorVersion
    }

    /** `find_filtered_jres(version, …)`. */
    async function findFilteredJres(version: number | null): Promise<JavaVersion[]> {
        await wait(260)
        return machine.javas
            .filter((java) => version == null || java.parsed_version === version)
            .map((java) => ({ ...java }))
    }

    /** `auto_install_java(version)`: downloads Mojang's runtime into the app's folder. */
    async function autoInstallJava(version: number, instanceId?: string): Promise<string> {
        await wait(2800)
        const path = machine.managedRuntimePath(preview.instances.get(instanceId ?? ''), version)
        machine.installRuntime(path, version)
        return path
    }

    return { getJre, testJre, findFilteredJres, autoInstallJava }
}

export function useJavaTest(jre: ReturnType<typeof useJre>) {
    const preview = usePreview()
    const testingJava = ref(false)
    const javaTestResult = ref<boolean | null>(null)
    let cancelDebounce: (() => void) | null = null

    async function runJavaTest(path: string, version: number | null) {
        cancelDebounce?.()
        cancelDebounce = null
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
        cancelDebounce?.()
        if (!path) {
            javaTestResult.value = null
            return
        }
        cancelDebounce = preview.clock.after(delay, () => runJavaTest(path, version))
    }

    async function testJavaInstallation(path: string, version: number | null) {
        await runJavaTest(path, version)
    }

    return {
        testingJava,
        javaTestResult,
        testJavaInstallationDebounced,
        testJavaInstallation,
    }
}
