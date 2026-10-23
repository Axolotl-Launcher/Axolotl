<script setup lang="ts">
import Checkbox from '@modrinth/ui/src/components/base/Checkbox.vue'
import Slider from '@modrinth/ui/src/components/base/Slider.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { injectNotificationManager } from '@modrinth/ui/src/providers/web-notifications.ts'
import { computed, ref, watch } from 'vue'

import { usePreview } from '../../../lib/state'
import type { GameInstance } from '../../../lib/types'
import { javaSettingsMessages as messages } from '../messages'
import { injectInstanceSettings } from './context'
import EnvironmentVariablesInput from './java/EnvironmentVariablesInput.vue'
import { collectGcContext, extractJavaMajorVersion, type GcContext } from './java/gc'
import JavaArgumentsInput from './java/JavaArgumentsInput.vue'
import JavaSelector from './java/JavaSelector.vue'
import {
    type JavaVersion,
    MACHINE_CPU_CORES,
    requiredJavaMajor,
    useSimulatedMachine,
} from './java/machine'
import { presentMods, useMemorySlider } from './java/memory'
import MemoryAllocationDisplay from './java/MemoryAllocationDisplay.vue'
import { useGlobalSettings, useInstanceSettingsState, useSimulatedInvoke } from './state'

/**
 * Port of `apps/app-frontend/src/components/ui/instance_settings/JavaSettings.vue`.
 * `get_optimal_jre_key` finds the runtime of the Java version the instance's
 * Minecraft version requires among the simulated Mac's runtimes; the GC context
 * counts the instance's installed mods.
 */
const { handleError } = injectNotificationManager()
const { formatMessage } = useVIntl()
const preview = usePreview()
const machine = useSimulatedMachine()
const settingsState = useInstanceSettingsState()
const simulate = useSimulatedInvoke()

const { instance } = injectInstanceSettings()
const instanceId = instance.value.id
const supportsMemoryOptimization = false

const { settings: globalSettings } = useGlobalSettings()
const extras = settingsState.extrasFor(instanceId)

/** `get_optimal_jre_key(instanceId)`: the runtime of the Java version the game version needs. */
async function getOptimalJreKey(): Promise<JavaVersion | null> {
    await simulate(140)
    const major = await requiredJavaMajor(instance.value.game_version, instance.value.loader)
    const java = machine.javas.find((entry) => entry.parsed_version === major)
    return java ? { ...java } : null
}

const optimalJava: Readonly<JavaVersion> | null = await getOptimalJreKey().catch((error) => {
    handleError(error)
    return null
})
const requiredJavaVersion = optimalJava?.parsed_version ?? null

const overrideJavaInstall = ref(!!instance.value.java_path)
const overrideJava = ref<Partial<JavaVersion> & { path: string }>({
    ...(optimalJava ?? {}),
    path: instance.value.java_path ?? optimalJava?.path ?? '',
})
const displayedJava = computed({
    get: () =>
        overrideJavaInstall.value ? overrideJava.value : (optimalJava ?? overrideJava.value),
    set: (value) => {
        overrideJava.value = value
    },
})

watch(overrideJavaInstall, (enabled) => {
    if (enabled && !overrideJava.value.path) {
        overrideJava.value = { ...(optimalJava ?? {}), path: optimalJava?.path ?? '' }
    }
})

const overrideJavaArgs = ref((instance.value.extra_launch_args?.length ?? 0) > 0)
const javaArgs = ref(
    (instance.value.extra_launch_args ?? globalSettings?.extra_launch_args ?? []).join(' '),
)

const overrideEnvVars = ref(extras.custom_env_vars != null)
const envVars = ref<[string, string][]>(
    extras.custom_env_vars ?? globalSettings?.custom_env_vars ?? [],
)

const defaultMemory = { maximum: 2048, automatic: true, optimize_before_launch: false }
const overrideMemorySettings = ref(!!instance.value.memory)
const memory = ref({
    ...defaultMemory,
    ...(instance.value.memory ?? globalSettings?.memory),
})
const effectiveMemory = computed(() =>
    overrideMemorySettings.value ? memory.value : { ...defaultMemory, ...globalSettings?.memory },
)
const memData = useMemorySlider()
const maxMemory = memData.maxMemory
const snapPoints = memData.snapPoints

const gcContext = ref<GcContext | null>(null)

async function updateGcContext() {
    const javaMajorVersion = extractJavaMajorVersion(displayedJava.value?.parsed_version)
    let modCount = 0
    try {
        await preview.downloads.ensureContents()
        modCount = presentMods(preview.downloads.contents.value[instance.value.id] ?? [])
    } catch {
        modCount = 0
    }
    gcContext.value = collectGcContext(
        memory.value.maximum,
        instance.value.loader,
        MACHINE_CPU_CORES,
        javaMajorVersion,
        modCount,
    )
}

await updateGcContext()

watch([memory, displayedJava, () => instance.value.loader], updateGcContext)

const editInstanceObject = computed(() => ({
    java_path:
        overrideJavaInstall.value && overrideJava.value.path
            ? overrideJava.value.path.replace('java.exe', 'javaw.exe')
            : null,
    extra_launch_args: overrideJavaArgs.value
        ? javaArgs.value.trim().split(/\s+/).filter(Boolean)
        : null,
    memory: overrideMemorySettings.value ? memory.value : null,
}))

/** `edit(instanceId, patch)`. */
async function edit(
    id: string,
    patch: Partial<GameInstance> & { custom_env_vars?: [string, string][] | null },
) {
    await simulate()
    const target = preview.instances.get(id)
    if (!target) throw new Error('Unknown instance')
    const { custom_env_vars: customEnvVars, ...instancePatch } = patch
    Object.assign(target, instancePatch)
    if (customEnvVars !== undefined) settingsState.extrasFor(id).custom_env_vars = customEnvVars
}

let saveQueue = Promise.resolve()
function saveInstancePatch(
    patch: Partial<GameInstance> & { custom_env_vars?: [string, string][] | null },
) {
    const snapshot = JSON.parse(JSON.stringify(patch)) as typeof patch
    saveQueue = saveQueue.then(() => edit(instanceId, snapshot)).catch(handleError)
}

watch(
    [overrideEnvVars, envVars],
    () => {
        saveInstancePatch({ custom_env_vars: overrideEnvVars.value ? envVars.value : null })
    },
    { deep: true },
)

watch(
    [overrideJavaInstall, overrideJava, overrideJavaArgs, javaArgs, overrideMemorySettings, memory],
    () => {
        saveInstancePatch(editInstanceObject.value)
    },
    { deep: true },
)
</script>

<template>
    <div>
        <h2 class="m-0 mb-2 block text-base font-extrabold text-[var(--color-text-primary)]">
            {{ formatMessage(messages.javaInstallation) }}
        </h2>
        <Checkbox
            v-model="overrideJavaInstall"
            :label="formatMessage(messages.customJavaInstallation)"
            class="mb-2"
        />
        <JavaSelector
            v-model="displayedJava"
            :disabled="!overrideJavaInstall"
            :placeholder="formatMessage(messages.javaPathPlaceholder)"
            :version="requiredJavaVersion"
            :instance-id="instance.id"
            select-all-versions
        />

        <h2 class="mb-1 mt-4 block text-base font-extrabold text-[var(--color-text-primary)]">
            {{ formatMessage(messages.javaMemory) }}
        </h2>
        <Checkbox
            v-model="overrideMemorySettings"
            :label="formatMessage(messages.customMemoryAllocation)"
            class="mb-2"
        />
        <Checkbox
            v-if="overrideMemorySettings"
            v-model="memory.automatic"
            :label="formatMessage(messages.automaticMemory)"
            class="mb-2"
        />
        <div
            v-if="supportsMemoryOptimization && overrideMemorySettings"
            class="mb-2 flex flex-col gap-1"
        >
            <Checkbox
                v-model="memory.optimize_before_launch"
                :label="formatMessage(messages.optimizeMemoryBeforeLaunch)"
            />
            <p class="m-0 text-xs leading-tight text-[var(--color-text-tertiary)]">
                {{ formatMessage(messages.optimizeMemoryBeforeLaunchDescription) }}
            </p>
        </div>
        <Slider
            id="max-memory"
            v-model="memory.maximum"
            :disabled="!overrideMemorySettings || memory.automatic"
            :min="512"
            :max="maxMemory"
            :step="64"
            :snap-points="snapPoints"
            :snap-range="512"
            unit="MB"
        />
        <MemoryAllocationDisplay :instance-id="instance.id" :memory="effectiveMemory" />

        <h2 class="mb-1 mt-4 block text-base font-extrabold text-[var(--color-text-primary)]">
            {{ formatMessage(messages.javaArguments) }}
        </h2>
        <Checkbox
            v-model="overrideJavaArgs"
            :label="formatMessage(messages.customJavaArguments)"
            class="my-1"
        />
        <JavaArgumentsInput
            id="java-args"
            v-model="javaArgs"
            :disabled="!overrideJavaArgs"
            :gc-context="gcContext"
            :show-auto-details="true"
            :placeholder="formatMessage(messages.enterJavaArguments)"
        />

        <h2 class="mb-1 mt-4 block text-base font-extrabold text-[var(--color-text-primary)]">
            {{ formatMessage(messages.javaEnvironmentVariables) }}
        </h2>
        <Checkbox
            v-model="overrideEnvVars"
            :label="formatMessage(messages.customEnvironmentVariables)"
            class="mb-2"
        />
        <EnvironmentVariablesInput
            id="env-vars"
            v-model="envVars"
            autocomplete="off"
            :disabled="!overrideEnvVars"
            :placeholder="formatMessage(messages.enterEnvironmentVariables)"
        />
    </div>
</template>
