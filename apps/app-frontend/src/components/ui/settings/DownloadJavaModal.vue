<script setup>
import { ArrowLeftIcon, CoffeeIcon, SpinnerIcon, XIcon } from '@modrinth/assets'
import {
    Button,
    commonMessages,
    defineMessages,
    injectNotificationManager,
    NewModal,
    useVIntl,
} from '@modrinth/ui'
import { ref } from 'vue'

import AlibabaLogo from '@/assets/java-vendors/alibaba.png'
import AmazonLogo from '@/assets/java-vendors/amazon.png'
import AzulLogo from '@/assets/java-vendors/azul.png'
import BellSoftLogo from '@/assets/java-vendors/bellsoft.png'
import EclipseLogo from '@/assets/java-vendors/eclipse.png'
import GraalVmLogo from '@/assets/java-vendors/graalvm.webp'
import IbmLogo from '@/assets/java-vendors/ibm.webp'
import JetBrainsLogo from '@/assets/java-vendors/jetbrains.webp'
import MicrosoftLogo from '@/assets/java-vendors/microsoft.png'
import OracleLogo from '@/assets/java-vendors/oracle.png'
import SapLogo from '@/assets/java-vendors/sap.png'
import { trackEvent } from '@/helpers/analytics'
import { download_java, list_java_feed_vendors, list_java_feed_versions } from '@/helpers/jre'

const { handleError } = injectNotificationManager()
const { formatMessage } = useVIntl()

const messages = defineMessages({
    downloadJava: { id: 'app.settings.java.download.title', defaultMessage: 'Download Java' },
    selectVendor: {
        id: 'app.settings.java.download.select-vendor',
        defaultMessage: 'Choose a distribution:',
    },
    selectVersion: {
        id: 'app.settings.java.download.select-version-feed',
        defaultMessage: 'Select a version of {vendor}:',
    },
    back: { id: 'app.settings.java.download.back', defaultMessage: 'Back to distributions' },
    loading: { id: 'app.settings.java.download.loading', defaultMessage: 'Loading...' },
    noVendors: {
        id: 'app.settings.java.download.no-vendors',
        defaultMessage: 'No distributions available.',
    },
    noVersions: {
        id: 'app.settings.java.download.no-versions',
        defaultMessage: 'No versions available.',
    },
    versionLabel: {
        id: 'app.settings.java.download.version-label',
        defaultMessage: 'Java {version}',
    },
})

const vendorBranding = {
    Alibaba: { logo: AlibabaLogo, product: 'Dragonwell' },
    Amazon: { logo: AmazonLogo, product: 'Corretto' },
    Azul: { logo: AzulLogo, product: 'Zulu' },
    BellSoft: { logo: BellSoftLogo, product: 'Liberica JDK' },
    Eclipse: { logo: EclipseLogo, product: 'Temurin' },
    GraalVM: { logo: GraalVmLogo, product: 'Community Edition' },
    IBM: { logo: IbmLogo, product: 'Semeru' },
    JetBrains: { logo: JetBrainsLogo, product: 'Runtime' },
    Microsoft: { logo: MicrosoftLogo, product: 'OpenJDK' },
    Oracle: { logo: OracleLogo, product: 'OpenJDK / GraalVM' },
    SAP: { logo: SapLogo, product: 'SapMachine' },
}

const emit = defineEmits(['downloaded'])

const modal = ref(null)
const loading = ref(false)
const vendors = ref([])
const selectedVendor = ref(null)
const versions = ref([])
const downloading = ref(null)
let requestId = 0

async function show() {
    const currentRequestId = ++requestId
    selectedVendor.value = null
    versions.value = []
    downloading.value = null
    loading.value = true
    vendors.value = []
    modal.value?.show()

    const result = await list_java_feed_vendors().catch(handleError)
    if (currentRequestId !== requestId) return

    vendors.value = result || []
    loading.value = false
}

defineExpose({ show })

async function selectVendor(vendor) {
    const currentRequestId = ++requestId
    selectedVendor.value = vendor
    loading.value = true
    versions.value = []

    const result = await list_java_feed_versions(vendor).catch(handleError)
    if (currentRequestId !== requestId || selectedVendor.value !== vendor) return

    versions.value = result || []
    loading.value = false
}

function backToVendors() {
    requestId += 1
    selectedVendor.value = null
    versions.value = []
    loading.value = false
}

async function downloadVersion(info) {
    downloading.value = info.major_version
    trackEvent('JavaDownload', { vendor: info.vendor, version: info.major_version })
    modal.value?.hide()

    const job = await download_java(info.vendor, info.major_version).catch(handleError)
    downloading.value = null

    if (job) {
        emit('downloaded', job)
    }
}
</script>

<template>
    <NewModal
        ref="modal"
        :header="formatMessage(messages.downloadJava)"
        :closable="downloading === null"
        max-width="720px"
        width="min(720px, calc(100vw - 2rem))"
        max-content-height="min(34rem, 70vh)"
        scrollable
        actions-divider
    >
        <div class="flex min-h-40 flex-col gap-4">
            <template v-if="!selectedVendor">
                <span class="font-semibold text-[var(--color-text-primary)]">{{
                    formatMessage(messages.selectVendor)
                }}</span>
                <div
                    v-if="loading"
                    class="flex min-h-32 items-center justify-center gap-2 text-sm text-[var(--color-text-tertiary)]"
                    role="status"
                >
                    <SpinnerIcon class="size-4 animate-spin" aria-hidden="true" />
                    {{ formatMessage(messages.loading) }}
                </div>
                <div
                    v-else-if="vendors.length === 0"
                    class="flex min-h-32 items-center justify-center text-sm text-[var(--color-text-tertiary)]"
                >
                    {{ formatMessage(messages.noVendors) }}
                </div>
                <div v-else class="grid grid-cols-2 gap-2 sm:grid-cols-3">
                    <Button
                        v-for="vendor in vendors"
                        :key="vendor"
                        type="base"
                        class="!h-16 !w-full !min-w-0 !justify-start !gap-3 !rounded-lg !px-3 !py-2 !text-left !shadow-none"
                        native-type="button"
                        @click="selectVendor(vendor)"
                    >
                        <span
                            class="flex size-10 shrink-0 items-center justify-center overflow-hidden rounded-md p-1"
                        >
                            <img
                                v-if="vendorBranding[vendor]"
                                :src="vendorBranding[vendor].logo"
                                alt=""
                                class="size-full object-contain"
                            />
                            <CoffeeIcon
                                v-else
                                class="size-5 text-[var(--color-text-tertiary)]"
                                aria-hidden="true"
                            />
                        </span>
                        <span
                            class="flex min-w-0 flex-1 flex-col items-start text-left leading-tight"
                        >
                            <span
                                class="w-full truncate text-left text-sm font-semibold text-[var(--color-text-primary)]"
                                >{{ vendor }}</span
                            >
                            <span
                                v-if="vendorBranding[vendor]"
                                class="w-full truncate text-left text-xs font-normal text-[var(--color-text-tertiary)]"
                            >
                                {{ vendorBranding[vendor].product }}
                            </span>
                        </span>
                    </Button>
                </div>
            </template>

            <template v-else>
                <div class="flex items-center gap-3">
                    <span
                        class="flex size-10 shrink-0 items-center justify-center overflow-hidden rounded-md p-1"
                    >
                        <img
                            v-if="vendorBranding[selectedVendor]"
                            :src="vendorBranding[selectedVendor].logo"
                            alt=""
                            class="size-full object-contain"
                        />
                        <CoffeeIcon
                            v-else
                            class="size-5 text-[var(--color-text-tertiary)]"
                            aria-hidden="true"
                        />
                    </span>
                    <span class="min-w-0 font-semibold text-[var(--color-text-primary)]">
                        {{ formatMessage(messages.selectVersion, { vendor: selectedVendor }) }}
                    </span>
                </div>
                <div
                    v-if="loading"
                    class="flex min-h-32 items-center justify-center gap-2 text-sm text-[var(--color-text-tertiary)]"
                    role="status"
                >
                    <SpinnerIcon class="size-4 animate-spin" aria-hidden="true" />
                    {{ formatMessage(messages.loading) }}
                </div>
                <div
                    v-else-if="versions.length === 0"
                    class="flex min-h-32 items-center justify-center text-sm text-[var(--color-text-tertiary)]"
                >
                    {{ formatMessage(messages.noVersions) }}
                </div>
                <div v-else class="grid grid-cols-2 gap-2 sm:grid-cols-4">
                    <Button
                        v-for="info in versions"
                        :key="info.major_version"
                        type="base"
                        class="!h-12 !w-full !min-w-0 !rounded-lg !px-3 !shadow-none"
                        :disabled="downloading !== null"
                        native-type="button"
                        @click="downloadVersion(info)"
                    >
                        <SpinnerIcon
                            v-if="downloading === info.major_version"
                            class="animate-spin"
                            aria-hidden="true"
                        />
                        <CoffeeIcon v-else aria-hidden="true" />
                        <span class="truncate text-sm font-semibold tabular-nums">
                            {{
                                formatMessage(messages.versionLabel, {
                                    version: info.major_version,
                                })
                            }}
                        </span>
                    </Button>
                </div>
            </template>
        </div>

        <template #actions>
            <div class="flex flex-wrap justify-end gap-2">
                <Button
                    v-if="selectedVendor"
                    type="outlined"
                    :disabled="downloading !== null"
                    native-type="button"
                    @click="backToVendors"
                >
                    <ArrowLeftIcon aria-hidden="true" />
                    {{ formatMessage(messages.back) }}
                </Button>
                <Button
                    type="outlined"
                    :disabled="downloading !== null"
                    native-type="button"
                    @click="modal?.hide()"
                >
                    <XIcon aria-hidden="true" />
                    {{ formatMessage(commonMessages.cancelButton) }}
                </Button>
            </div>
        </template>
    </NewModal>
</template>
