import { reactive } from 'vue'

import { previewDomain, type PreviewState, usePreviewDomain } from '../../../../lib/state'
import type { GameInstance } from '../../../../lib/types'

/*
 * The Mac the preview window runs on, as the Java and memory settings see it:
 * the Java runtimes `find_filtered_jres` detects, the RAM `get_max_memory` and
 * `get_memory_status` report, and the CPU `navigator.hardwareConcurrency` counts.
 * Paths are the standard macOS locations; versions and sizes are example values
 * of this machine.
 */

/** `JavaVersion` of `apps/app-frontend/src/helpers/types.d.ts`. */
export interface JavaVersion {
    parsed_version: number
    version: string
    architecture: string
    path: string
    distribution?: string | null
}

const GIB = 1024 ** 3

export const MACHINE_TOTAL_MEMORY = 16 * GIB
export const MACHINE_CPU_CORES = 10

function temurin(major: number, version: string, architecture = 'aarch64'): JavaVersion {
    return {
        parsed_version: major,
        version,
        architecture,
        path: `/Library/Java/JavaVirtualMachines/temurin-${major}.jdk/Contents/Home/bin/java`,
        distribution: 'temurin',
    }
}

function createMachine(state: PreviewState) {
    const javas = reactive<JavaVersion[]>([
        temurin(25, '25.0.1'),
        temurin(21, '21.0.7'),
        temurin(17, '17.0.15'),
        temurin(8, '1.8.0_452', 'x86_64'),
    ])
    /** macOS reports free + inactive + purgeable pages as available. */
    let idleAvailable = 7.4 * GIB
    let drift = 0

    function sampleAvailable() {
        drift = Math.max(
            -0.35 * GIB,
            Math.min(0.35 * GIB, drift + (Math.random() - 0.5) * 0.12 * GIB),
        )
        // A running game holds its heap.
        const running = state.processes.processes.value.reduce(
            (sum, process) => sum + (process.instance.memory?.maximum ?? 2048) * 1024 * 1024,
            0,
        )
        return Math.max(0.6 * GIB, idleAvailable + drift - running)
    }

    /** The app's own runtimes, `<config dir>/meta/java_versions/…` (`auto_install_java`). */
    function managedRuntimePath(instance: GameInstance | undefined, major: number) {
        const profiles =
            instance?.path.replace(/\/profiles\/[^/]+$/, '') ??
            '/Users/steve/Library/Application Support/red.ghs.axolotl'
        const component =
            major === 8
                ? 'jre-legacy'
                : major === 16
                  ? 'java-runtime-alpha'
                  : major === 17
                    ? 'java-runtime-gamma'
                    : major === 21
                      ? 'java-runtime-delta'
                      : 'java-runtime-epsilon'
        return `${profiles}/meta/java_versions/mojang-${component}-mac-os-arm64/jre.bundle/Contents/Home/bin/java`
    }

    function installRuntime(path: string, major: number) {
        if (!javas.some((java) => java.path === path)) {
            javas.unshift({
                parsed_version: major,
                version: `${major}`,
                architecture: 'aarch64',
                path,
                distribution: 'mojang',
            })
        }
        idleAvailable = Math.max(6.6 * GIB, idleAvailable - 0.05 * GIB)
    }

    return {
        value: { javas, sampleAvailable, managedRuntimePath, installRuntime },
    }
}

export type SimulatedMachine = ReturnType<typeof createMachine>['value']

export function useSimulatedMachine(): SimulatedMachine {
    return usePreviewDomain('instance-settings:machine', createMachine)
}

/** The machine of a given window, for code outside a component's setup. */
export function simulatedMachine(state: PreviewState): SimulatedMachine {
    return previewDomain(state, 'instance-settings:machine', createMachine)
}

/**
 * The Java major version Mojang's version manifest declares for a game version
 * (`javaVersion.majorVersion`), by the release the version belongs to.
 */
export async function requiredJavaMajor(gameVersion: string, loader: string): Promise<number> {
    if (loader === 'lite_loader') return 8
    const data = await import('../../../../data')
    const minecraft = await data.loadMinecraft()
    const all = [...minecraft.releases, ...minecraft.snapshots, ...(minecraft.old ?? [])]
    const time = (id: string) => {
        const entry = all.find((version) => version.id === id)
        return entry ? Date.parse(entry.releaseTime) : Number.NaN
    }
    const released = time(gameVersion)
    let major = 8
    if (!Number.isNaN(released)) {
        if (released >= time('26.1')) major = 25
        else if (released >= time('1.20.5')) major = 21
        else if (released >= time('1.18')) major = 17
        else if (released >= time('1.17')) major = 16
    }
    if (loader === 'cleanroom' && major < 21) major = 21
    return major
}
