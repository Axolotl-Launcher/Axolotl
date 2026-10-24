<template>
    <JavaDetectionModal ref="detectJavaModal" @submit="commitSelection" />
    <div
        :id="props.id"
        class="flex flex-wrap justify-between items-center gap-2"
        :class="{ compact }"
    >
        <div class="flex items-center gap-2 w-full min-w-0">
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
                            path: val,
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
                <SpinnerIcon v-if="testingJava" class="animate-spin h-4 w-4" />
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
        <span class="flex items-center gap-2 m-0">
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
    </div>
</template>

<script setup>
import {
    CheckCircleIcon,
    DownloadIcon,
    FolderSearchIcon,
    RefreshCwIcon,
    SearchIcon,
    SpinnerIcon,
    XCircleIcon,
} from '@modrinth/assets'
import {
    Button,
    commonMessages,
    defineMessages,
    IconButton,
    injectNotificationManager,
    StyledInput,
    useVIntl,
} from '@modrinth/ui'
import { open } from '@tauri-apps/plugin-dialog'
import { computed, ref, watch } from 'vue'

import JavaDetectionModal from '@/components/ui/JavaDetectionModal.vue'
import useJavaTest from '@/composables/useJavaTest'
import { trackEvent } from '@/helpers/analytics'
import { auto_install_java, find_filtered_jres, get_jre } from '@/helpers/jre.js'

const { handleError } = injectNotificationManager()
const { formatMessage } = useVIntl()

const messages = defineMessages({
    alreadyInstalled: { id: 'app.java.already-installed', defaultMessage: 'Already installed' },
    installRecommended: {
        id: 'app.java.install-recommended',
        defaultMessage: 'Install recommended',
    },
    detect: { id: 'app.java.detect', defaultMessage: 'Detect' },
    select: { id: 'app.java.select', defaultMessage: 'Select' },
    browse: { id: 'app.java.browse', defaultMessage: 'Browse' },
    browseForExecutable: {
        id: 'app.java.browse-for-executable',
        defaultMessage: 'Browse for Java executable',
    },
    testInstallation: {
        id: 'app.java.test-installation',
        defaultMessage: 'Test Java installation',
    },
})

const props = defineProps({
    id: {
        type: String,
        required: false,
        default: null,
    },
    version: {
        type: Number,
        required: false,
        default: null,
    },
    modelValue: {
        type: Object,
        default: () => ({
            path: '',
            version: '',
        }),
    },
    disabled: {
        type: Boolean,
        required: false,
        default: false,
    },
    placeholder: {
        type: String,
        required: false,
        default: null,
    },
    compact: {
        type: Boolean,
        default: false,
    },
    selectAllVersions: {
        type: Boolean,
        default: false,
    },
})

const emit = defineEmits(['update:modelValue', 'commit'])

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

async function runTest(path) {
    await testJavaInstallation(path, testVersion.value, true)
    if (props.version != null) {
        await recommendedJavaTest.testJavaInstallation(path, props.version, false)
    }
}

function commitSelection(javaVersion) {
    emit('update:modelValue', javaVersion)
    emit('commit', javaVersion)
}

watch(
    [() => props.modelValue?.path, () => props.version, () => props.selectAllVersions],
    ([newPath]) => {
        if (newPath) {
            if (!hasInitialized) {
                testJavaInstallation(newPath, testVersion.value, false)
                if (props.version != null) {
                    recommendedJavaTest.testJavaInstallation(newPath, props.version, false)
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

async function handleJavaFileInput() {
    const filePath = await open()

    if (filePath) {
        let result = await get_jre(filePath.path ?? filePath).catch(handleError)
        if (!result) {
            result = {
                path: filePath.path ?? filePath,
                version: props.version?.toString() ?? '',
                parsed_version: props.version ?? 0,
                architecture: 'x86',
            }
        }

        trackEvent('JavaManualSelect', {
            version: props.version,
        })

        commitSelection(result)
    }
}

const detectJavaModal = ref(null)
async function autoDetect() {
    const filterVersion = props.selectAllVersions ? null : props.version
    if (!props.compact) {
        detectJavaModal.value.show(filterVersion, props.modelValue)
    } else {
        const versions = await find_filtered_jres(filterVersion, false, false).catch(handleError)
        if (versions?.length > 0) {
            commitSelection(versions[0])
        }
    }
}

async function reinstallJava() {
    installingJava.value = true
    try {
        const path = await auto_install_java(props.version).catch(handleError)
        if (!path) return

        let result = await get_jre(path).catch(handleError)
        if (!result) {
            result = {
                path: path,
                version: props.version?.toString() ?? '',
                parsed_version: props.version ?? 0,
                architecture: 'x86',
            }
        }

        trackEvent('JavaReInstall', { path: path, version: props.version })
        commitSelection(result)
        runTest(result.path)
    } finally {
        installingJava.value = false
    }
}
</script>

<style lang="scss" scoped></style>
