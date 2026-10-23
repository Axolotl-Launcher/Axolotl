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
import { computed, ref, watch } from 'vue'

import { messages } from '../../messages'
import JavaDetectionModal from './JavaDetectionModal.vue'
import { type JavaVersion, useJre } from './jre'
import MacOpenPanel from './MacOpenPanel.vue'
import useJavaTest from './useJavaTest'

/**
 * A Java path field with its test, "Install recommended", "Detect" and "Browse"
 * actions (apps/app-frontend/src/components/ui/JavaSelector.vue). The native file
 * dialog is the simulated macOS open panel.
 */
const { handleError } = injectNotificationManager()
const { formatMessage } = useVIntl()
const jre = useJre()

/** `.catch(handleError)`: shows the error toast and resolves to nothing. */
function reportError(error: unknown): null {
    handleError(error)
    return null
}

const props = withDefaults(
    defineProps<{
        id?: string | null
        version?: number | null
        modelValue?: Partial<JavaVersion> | null
        disabled?: boolean
        placeholder?: string | null
        compact?: boolean
        selectAllVersions?: boolean
    }>(),
    {
        id: null,
        version: null,
        modelValue: () => ({ path: '', version: '' }),
        disabled: false,
        placeholder: null,
        compact: false,
        selectAllVersions: false,
    },
)

const emit = defineEmits<{
    'update:modelValue': [Partial<JavaVersion> | null | undefined]
    commit: [Partial<JavaVersion> | null | undefined]
}>()

const {
    testingJava,
    javaTestResult: testingJavaSuccess,
    testJavaInstallationDebounced,
    testJavaInstallation,
} = useJavaTest()
const recommendedJavaTest = useJavaTest()

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
    await testJavaInstallation(path ?? '', testVersion.value, true)
    if (props.version != null) {
        await recommendedJavaTest.testJavaInstallation(path ?? '', props.version, false)
    }
}

function commitSelection(javaVersion: Partial<JavaVersion>) {
    emit('update:modelValue', javaVersion)
    emit('commit', javaVersion)
}

watch(
    () => props.modelValue?.path,
    (newPath) => {
        if (newPath) {
            if (!hasInitialized) {
                void testJavaInstallation(newPath, testVersion.value, false)
                if (props.version != null) {
                    void recommendedJavaTest.testJavaInstallation(newPath, props.version, false)
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

const openPanel = ref<InstanceType<typeof MacOpenPanel> | null>(null)

async function handleJavaFileInput() {
    const filePath = await openPanel.value?.pick()

    if (filePath) {
        const result: Partial<JavaVersion> = (await jre.getJre(filePath).catch(reportError)) ?? {
            path: filePath,
            version: props.version?.toString() ?? '',
            parsed_version: props.version ?? 0,
            architecture: 'x86',
        }

        commitSelection(result)
    }
}

const detectJavaModal = ref<InstanceType<typeof JavaDetectionModal> | null>(null)
async function autoDetect() {
    const filterVersion = props.selectAllVersions ? null : props.version
    if (!props.compact) {
        void detectJavaModal.value?.show(filterVersion, props.modelValue)
    } else {
        const versions = await jre.findFilteredJres(filterVersion, false, false).catch(reportError)
        if (versions && versions.length > 0) {
            commitSelection(versions[0])
        }
    }
}

async function reinstallJava() {
    installingJava.value = true
    try {
        const path = await jre.autoInstallJava(props.version ?? 0).catch(reportError)
        if (!path) return

        const result: Partial<JavaVersion> = (await jre.getJre(path).catch(reportError)) ?? {
            path,
            version: props.version?.toString() ?? '',
            parsed_version: props.version ?? 0,
            architecture: 'x86',
        }

        commitSelection(result)
        void runTest(result.path)
    } finally {
        installingJava.value = false
    }
}
</script>

<template>
    <JavaDetectionModal ref="detectJavaModal" @submit="commitSelection" />
    <MacOpenPanel ref="openPanel" />
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
                :label="formatMessage(messages.appJavaTestInstallation)"
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
                    recommendedInstalled
                        ? formatMessage(messages.appJavaAlreadyInstalled)
                        : undefined
                "
                class="!shadow-none"
                :aria-label="formatMessage(messages.appJavaInstallRecommended)"
                :disabled="props.disabled || installingJava || recommendedInstalled"
                @click="reinstallJava"
                ><DownloadIcon />
                {{
                    installingJava
                        ? formatMessage(commonMessages.installingLabel)
                        : formatMessage(messages.appJavaInstallRecommended)
                }}
            </Button>
            <Button
                class="!shadow-none"
                :aria-label="
                    formatMessage(
                        props.selectAllVersions ? messages.appJavaSelect : messages.appJavaDetect,
                    )
                "
                :disabled="props.disabled"
                @click="autoDetect"
                ><SearchIcon />
                {{
                    formatMessage(
                        props.selectAllVersions ? messages.appJavaSelect : messages.appJavaDetect,
                    )
                }}
            </Button>
            <Button
                class="!shadow-none"
                :aria-label="formatMessage(messages.appJavaBrowseForExecutable)"
                :disabled="props.disabled"
                @click="handleJavaFileInput()"
                ><FolderSearchIcon />
                {{ formatMessage(messages.appJavaBrowse) }}
            </Button>
        </span>
    </div>
</template>
