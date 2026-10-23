<script setup lang="ts">
import CheckCircleIcon from '@modrinth/assets/icons/check-circle.svg?component'
import DownloadIcon from '@modrinth/assets/icons/download.svg?component'
import FolderSearchIcon from '@modrinth/assets/icons/folder-search.svg?component'
import RefreshCwIcon from '@modrinth/assets/icons/refresh-cw.svg?component'
import SearchIcon from '@modrinth/assets/icons/search.svg?component'
import SpinnerIcon from '@modrinth/assets/icons/spinner.svg?component'
import XCircleIcon from '@modrinth/assets/icons/x-circle.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import IconButton from '@modrinth/ui/src/components/base/buttons/IconButton.vue'
import StyledInput from '@modrinth/ui/src/components/base/StyledInput.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { injectNotificationManager } from '@modrinth/ui/src/providers/web-notifications.ts'
import { commonMessages } from '@modrinth/ui/src/utils/common-messages.ts'
import { computed, ref, useTemplateRef, watch } from 'vue'

import { javaSelectorMessages as messages } from '../../messages'
import JavaDetectionModal from './JavaDetectionModal.vue'
import { useJavaTest, useJre } from './jre'
import { type JavaVersion, useSimulatedMachine } from './machine'

/**
 * Port of `apps/app-frontend/src/components/ui/JavaSelector.vue`. Detection,
 * testing and "Install recommended" run against the simulated Mac's runtimes;
 * "Browse" opens the browser's file picker in place of the system dialog.
 */
const { handleError } = injectNotificationManager()
const { formatMessage } = useVIntl()
const machine = useSimulatedMachine()
const jre = useJre(machine)

const props = withDefaults(
    defineProps<{
        id?: string | null
        version?: number | null
        modelValue?: Partial<JavaVersion> & { path: string }
        disabled?: boolean
        placeholder?: string | null
        compact?: boolean
        selectAllVersions?: boolean
        instanceId?: string | null
    }>(),
    {
        id: null,
        version: null,
        modelValue: () => ({ path: '', version: '' }),
        disabled: false,
        placeholder: null,
        compact: false,
        selectAllVersions: false,
        instanceId: null,
    },
)

const emit = defineEmits<{
    'update:modelValue': [value: Partial<JavaVersion> & { path: string }]
    commit: [value: (Partial<JavaVersion> & { path: string }) | undefined]
}>()

const {
    testingJava,
    javaTestResult: testingJavaSuccess,
    testJavaInstallationDebounced,
    testJavaInstallation,
} = useJavaTest(jre)
const recommendedJavaTest = useJavaTest(jre)

const installingJava = ref(false)
const hoveringTest = ref(false)
const testVersion = computed(() => (props.selectAllVersions ? null : props.version))
const recommendedInstalled = computed(() => {
    if (props.version == null) return false
    if (props.selectAllVersions) return recommendedJavaTest.javaTestResult.value === true
    return testingJavaSuccess.value === true
})
let hasInitialized = false

async function runTest(path: string | undefined) {
    await testJavaInstallation(path ?? '', testVersion.value)
    if (props.version != null) {
        await recommendedJavaTest.testJavaInstallation(path ?? '', props.version)
    }
}

function commitSelection(javaVersion: Partial<JavaVersion> & { path: string }) {
    emit('update:modelValue', javaVersion)
    emit('commit', javaVersion)
}

watch(
    () => props.modelValue?.path,
    (newPath) => {
        if (newPath) {
            if (!hasInitialized) {
                testJavaInstallation(newPath, testVersion.value)
                if (props.version != null) {
                    recommendedJavaTest.testJavaInstallation(newPath, props.version)
                }
                hasInitialized = true
            } else {
                testJavaInstallationDebounced(newPath, testVersion.value)
                if (props.version != null) {
                    recommendedJavaTest.testJavaInstallationDebounced(newPath, props.version)
                }
            }
        }
    },
    { immediate: true },
)

const fileInput = useTemplateRef<HTMLInputElement>('fileInput')

/** `open()`: any file; the browser's picker reports the file's name. */
function openFile(): Promise<string | null> {
    const input = fileInput.value
    if (!input) return Promise.resolve(null)
    return new Promise((resolve) => {
        const done = (value: string | null) => {
            input.removeEventListener('change', onChange)
            input.removeEventListener('cancel', onCancel)
            resolve(value)
        }
        const onChange = () => {
            const file = input.files?.[0]
            input.value = ''
            done(file ? file.name : null)
        }
        const onCancel = () => done(null)
        input.addEventListener('change', onChange)
        input.addEventListener('cancel', onCancel)
        input.value = ''
        input.click()
    })
}

async function handleJavaFileInput() {
    const filePath = await openFile()

    if (filePath) {
        let result: (Partial<JavaVersion> & { path: string }) | null | void = await jre
            .getJre(filePath)
            .catch(handleError)
        if (!result) {
            result = {
                path: filePath,
                version: props.version?.toString() ?? '',
                parsed_version: props.version ?? 0,
                architecture: 'x86',
            }
        }

        commitSelection(result)
    }
}

const detectJavaModal = ref<InstanceType<typeof JavaDetectionModal> | null>(null)
async function autoDetect() {
    const filterVersion = props.selectAllVersions ? null : props.version
    if (!props.compact) {
        detectJavaModal.value?.show(filterVersion, props.modelValue)
    } else {
        const versions = await jre.findFilteredJres(filterVersion).catch(handleError)
        if (versions && versions.length > 0) {
            commitSelection(versions[0])
        }
    }
}

async function reinstallJava() {
    if (props.version == null) return
    installingJava.value = true
    try {
        const path = await jre
            .autoInstallJava(props.version, props.instanceId ?? undefined)
            .catch(handleError)
        if (!path) return

        let result: (Partial<JavaVersion> & { path: string }) | null | void = await jre
            .getJre(path)
            .catch(handleError)
        if (!result) {
            result = {
                path,
                version: props.version?.toString() ?? '',
                parsed_version: props.version ?? 0,
                architecture: 'x86',
            }
        }

        commitSelection(result)
        runTest(result.path)
    } finally {
        installingJava.value = false
    }
}
</script>

<template>
    <JavaDetectionModal ref="detectJavaModal" @submit="commitSelection" />
    <div
        :id="props.id ?? undefined"
        class="flex flex-wrap items-center justify-between gap-2"
        :class="{ compact }"
    >
        <div class="flex w-full min-w-0 items-center gap-2">
            <StyledInput
                autocomplete="off"
                :disabled="props.disabled"
                :model-value="props.modelValue ? props.modelValue.path : ''"
                :placeholder="placeholder ?? '/path/to/java'"
                wrapper-class="flex-1 min-w-0"
                @update:model-value="
                    (val) => {
                        emit('update:modelValue', {
                            ...props.modelValue,
                            path: String(val ?? ''),
                        })
                    }
                "
                @focusout="emit('commit', props.modelValue)"
            />
            <IconButton
                :label="formatMessage(messages.testInstallation)"
                :type="!hoveringTest && !testingJava ? 'colored-text' : 'base'"
                :color="
                    !hoveringTest && !testingJava
                        ? testingJavaSuccess === true
                            ? 'green'
                            : 'red'
                        : undefined
                "
                class="!shadow-none"
                :disabled="testingJava || props.disabled"
                @click="runTest(props.modelValue?.path)"
                @mouseenter="!props.disabled && (hoveringTest = true)"
                @mouseleave="hoveringTest = false"
            >
                <SpinnerIcon v-if="testingJava" class="h-4 w-4 animate-spin" />
                <CheckCircleIcon
                    v-else-if="testingJavaSuccess === true && !hoveringTest"
                    class="h-4 w-4"
                />
                <XCircleIcon
                    v-else-if="testingJavaSuccess !== true && !hoveringTest"
                    class="h-4 w-4"
                />
                <RefreshCwIcon v-else-if="!props.disabled" class="h-4 w-4" />
            </IconButton>
        </div>
        <span class="m-0 flex items-center gap-2">
            <Button
                v-if="props.version"
                v-tooltip="
                    recommendedInstalled ? formatMessage(messages.alreadyInstalled) : undefined
                "
                class="!shadow-none"
                :aria-label="formatMessage(messages.installRecommended)"
                :disabled="props.disabled || installingJava || recommendedInstalled"
                @click="reinstallJava"
                ><DownloadIcon />
                {{
                    installingJava
                        ? formatMessage(commonMessages.installingLabel)
                        : formatMessage(messages.installRecommended)
                }}
            </Button>
            <Button
                class="!shadow-none"
                :aria-label="
                    formatMessage(props.selectAllVersions ? messages.select : messages.detect)
                "
                :disabled="props.disabled"
                @click="autoDetect"
                ><SearchIcon />
                {{ formatMessage(props.selectAllVersions ? messages.select : messages.detect) }}
            </Button>
            <Button
                class="!shadow-none"
                :aria-label="formatMessage(messages.browseForExecutable)"
                :disabled="props.disabled"
                @click="handleJavaFileInput()"
                ><FolderSearchIcon />
                {{ formatMessage(messages.browse) }}
            </Button>
        </span>
        <input ref="fileInput" type="file" class="hidden" />
    </div>
</template>
