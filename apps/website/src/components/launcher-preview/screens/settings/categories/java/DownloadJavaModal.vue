<script setup lang="ts">
import ArrowLeftIcon from '@modrinth/assets/icons/arrow-left.svg?component'
import CoffeeIcon from '@modrinth/assets/icons/coffee.svg?component'
import SpinnerIcon from '@modrinth/assets/icons/spinner.svg?component'
import XIcon from '@modrinth/assets/icons/x.svg?component'
import Button from '@modrinth/ui/src/components/base/buttons/Button.vue'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import { injectNotificationManager } from '@modrinth/ui/src/providers/web-notifications.ts'
import { commonMessages } from '@modrinth/ui/src/utils/common-messages.ts'
import { ref } from 'vue'

import PreviewModal from '../../../../overlays/PreviewModal.vue'
import { messages } from '../../messages'
import { type JavaInstallJob, type JdkVersionInfo, useJre } from './jre'
import AlibabaLogo from './vendors/alibaba.png'
import AmazonLogo from './vendors/amazon.png'
import AzulLogo from './vendors/azul.png'
import BellSoftLogo from './vendors/bellsoft.png'
import EclipseLogo from './vendors/eclipse.png'
import GraalVmLogo from './vendors/graalvm.png'
import IbmLogo from './vendors/ibm.png'
import JetBrainsLogo from './vendors/jetbrains.png'
import MicrosoftLogo from './vendors/microsoft.png'
import OracleLogo from './vendors/oracle.png'
import SapLogo from './vendors/sap.png'

/**
 * "Download Java" (apps/app-frontend/src/components/ui/settings/DownloadJavaModal.vue):
 * pick a distribution, then a version; the download runs as an install job.
 */
const { handleError } = injectNotificationManager()
const { formatMessage } = useVIntl()
const jre = useJre()

const vendorBranding: Record<string, { logo: string; product: string }> = {
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

const emit = defineEmits<{ downloaded: [JavaInstallJob] }>()

const modal = ref<InstanceType<typeof PreviewModal> | null>(null)
const loading = ref(false)
const vendors = ref<string[]>([])
const selectedVendor = ref<string | null>(null)
const versions = ref<JdkVersionInfo[]>([])
const downloading = ref<number | null>(null)
let requestId = 0

async function show() {
    const currentRequestId = ++requestId
    selectedVendor.value = null
    versions.value = []
    downloading.value = null
    loading.value = true
    vendors.value = []
    modal.value?.show()

    const result = await jre.listJavaFeedVendors().catch(handleError)
    if (currentRequestId !== requestId) return

    vendors.value = result || []
    loading.value = false
}

defineExpose({ show })

async function selectVendor(vendor: string) {
    const currentRequestId = ++requestId
    selectedVendor.value = vendor
    loading.value = true
    versions.value = []

    const result = await jre.listJavaFeedVersions(vendor).catch(handleError)
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

async function downloadVersion(info: JdkVersionInfo) {
    downloading.value = info.major_version
    modal.value?.hide()

    const job = await jre.downloadJava(info.vendor, info.major_version).catch(handleError)
    downloading.value = null

    if (job) {
        emit('downloaded', job)
    }
}
</script>

<template>
    <PreviewModal
        ref="modal"
        :header="formatMessage(messages.appSettingsJavaDownloadTitle)"
        :closable="downloading === null"
        max-width="720px"
        width="720px"
        max-content-height="min(34rem, 560px)"
        scrollable
        actions-divider
    >
        <div class="flex min-h-40 flex-col gap-4">
            <template v-if="!selectedVendor">
                <span class="font-semibold text-[var(--color-text-primary)]">{{
                    formatMessage(messages.appSettingsJavaDownloadSelectVendor)
                }}</span>
                <div
                    v-if="loading"
                    class="flex min-h-32 items-center justify-center gap-2 text-sm text-[var(--color-text-tertiary)]"
                    role="status"
                >
                    <SpinnerIcon class="size-4 animate-spin" aria-hidden="true" />
                    {{ formatMessage(messages.appSettingsJavaDownloadLoading) }}
                </div>
                <div
                    v-else-if="vendors.length === 0"
                    class="flex min-h-32 items-center justify-center text-sm text-[var(--color-text-tertiary)]"
                >
                    {{ formatMessage(messages.appSettingsJavaDownloadNoVendors) }}
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
                                decoding="async"
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
                            decoding="async"
                            class="size-full object-contain"
                        />
                        <CoffeeIcon
                            v-else
                            class="size-5 text-[var(--color-text-tertiary)]"
                            aria-hidden="true"
                        />
                    </span>
                    <span class="min-w-0 font-semibold text-[var(--color-text-primary)]">
                        {{
                            formatMessage(messages.appSettingsJavaDownloadSelectVersionFeed, {
                                vendor: selectedVendor,
                            })
                        }}
                    </span>
                </div>
                <div
                    v-if="loading"
                    class="flex min-h-32 items-center justify-center gap-2 text-sm text-[var(--color-text-tertiary)]"
                    role="status"
                >
                    <SpinnerIcon class="size-4 animate-spin" aria-hidden="true" />
                    {{ formatMessage(messages.appSettingsJavaDownloadLoading) }}
                </div>
                <div
                    v-else-if="versions.length === 0"
                    class="flex min-h-32 items-center justify-center text-sm text-[var(--color-text-tertiary)]"
                >
                    {{ formatMessage(messages.appSettingsJavaDownloadNoVersions) }}
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
                                formatMessage(messages.appSettingsJavaDownloadVersionLabel, {
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
                    {{ formatMessage(messages.appSettingsJavaDownloadBack) }}
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
    </PreviewModal>
</template>
