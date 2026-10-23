<script setup lang="ts">
import { type Component, computed, useId } from 'vue'

import CodeFlowLogo from './CodeFlowLogo.vue'
import { getLobeIconComponent, hasLobeIcon } from './lobe-icon-components'
import {
    lobeModelAvatarBrands,
    lobeModelIconMappings,
    openAIModelBackgrounds,
} from './lobe-model-icons'
import { lobeProviderIcons } from './lobe-provider-icons'
import LobeBrandCombine from './LobeBrandCombine.vue'

/**
 * Provider and model marks of the AI settings (apps/app-frontend/src/
 * components/ui/settings/AIIcon.vue): LobeHub avatars, combined wordmarks and
 * model avatars, each SVG loaded on demand.
 */
const props = withDefaults(
    defineProps<{
        kind: 'model' | 'provider-avatar' | 'provider-combine' | 'provider-wordmark'
        value: string
        size?: number
    }>(),
    { size: 24 },
)

const iconCache = new Map<string, Component>()

// Lazy proxy: only requested lobehub SVGs are transformed/loaded.
const iconComponents = new Proxy({} as Record<string, Component | undefined>, {
    get(_target, prop) {
        return typeof prop === 'string' ? getLobeIconComponent(prop, iconCache) : undefined
    },
    has(_target, prop) {
        return typeof prop === 'string' && hasLobeIcon(prop)
    },
})

const soraGradientId = useId()
const providerConfig = computed(() => lobeProviderIcons[props.value.toLocaleLowerCase()])
const isCodeFlow = computed(() => props.value.toLocaleLowerCase() === 'codeflow')
const avatarConfig = computed(() => providerConfig.value?.avatar)
const avatarComponent = computed(() => {
    if (!providerConfig.value || !avatarConfig.value) return undefined
    const suffix = avatarConfig.value.asset === 'color' ? '-color' : ''
    return (
        iconComponents[`${providerConfig.value.slug}${suffix}`] ??
        iconComponents[providerConfig.value.slug]
    )
})
const combineSize = computed(() => props.size * (providerConfig.value?.combine.multiple ?? 1))
const combineBrand = computed(
    () => providerConfig.value?.combine.brand ?? providerConfig.value?.slug ?? props.value,
)
const specialAsset = computed(() => {
    switch (providerConfig.value?.combine.kind) {
        case 'google':
            return iconComponents['google-brand-color']
        case 'v0':
            return iconComponents.v0
        default:
            return undefined
    }
})
const modelMapping = computed(() => {
    const model = props.value.toLocaleLowerCase()
    return lobeModelIconMappings.find(({ keywords }) =>
        keywords.some((keyword) => new RegExp(keyword, 'i').test(model)),
    )
})
const modelAvatar = computed(() =>
    modelMapping.value ? lobeModelAvatarBrands[modelMapping.value.slug] : undefined,
)
const modelBackground = computed(() => {
    const openAIType = modelMapping.value?.openAIType
    return openAIType ? openAIModelBackgrounds[openAIType] : modelAvatar.value?.background
})
const modelComponent = computed(() => {
    if (!modelMapping.value || !modelAvatar.value) return undefined
    if (
        ['aihubmix', 'dalle', 'lg', 'nanobanana', 'sora', 'stepfun'].includes(
            modelMapping.value.slug,
        )
    ) {
        return undefined
    }
    const suffix = modelAvatar.value.asset === 'color' ? '-color' : ''
    return (
        iconComponents[`${modelMapping.value.slug}${suffix}`] ??
        iconComponents[modelMapping.value.slug]
    )
})
const modelAvatarStyle = computed(() => ({
    background: modelBackground.value ?? 'var(--surface-4)',
    color: modelAvatar.value?.color ?? 'var(--color-text-tertiary)',
    height: `${props.size}px`,
    width: `${props.size}px`,
}))
const avatarStyle = computed(() => ({
    background: avatarConfig.value?.background,
    borderRadius: `${Math.floor(props.size * 0.1)}px`,
    color:
        avatarConfig.value?.color ?? (isCodeFlow.value ? 'var(--color-text-primary)' : undefined),
    height: `${props.size}px`,
    width: `${props.size}px`,
}))
</script>

<template>
    <span
        v-if="kind === 'provider-avatar'"
        class="lobe-provider-avatar inline-flex flex-none items-center justify-center overflow-hidden"
        :class="{
            'black-background': avatarConfig?.background === '#000',
            'white-background': avatarConfig?.background === '#fff',
        }"
        :style="avatarStyle"
        aria-hidden="true"
    >
        <svg
            v-if="avatarConfig?.variant === 'ai302'"
            fill="currentColor"
            fill-rule="evenodd"
            viewBox="0 0 24 24"
            :style="{ transform: `scale(${avatarConfig.multiple})` }"
        >
            <path
                d="M11.88 21.5a4.49 4.49 0 01-2.772-.959 4.516 4.516 0 01-1.71-3.024 4.513 4.513 0 01-.007-1.086 4.46 4.46 0 01-.859.078A4.537 4.537 0 012 11.975c0-2.5 2.036-4.54 4.532-4.54.356 0 .7.041 1.034.12A4.543 4.543 0 0112.07 2.5c2.497 0 4.525 2.04 4.525 4.54 0 .145-.005.286-.02.43a4.596 4.596 0 011.125-.056 4.542 4.542 0 014.18 4.864 4.507 4.507 0 01-1.562 3.103 4.484 4.484 0 01-3.287 1.085 4.54 4.54 0 01-.647-.091c0 .01.007.019.007.028a4.522 4.522 0 01-.922 3.349 4.496 4.496 0 01-3.019 1.713 4.53 4.53 0 01-.57.035zm-2.512-5.993a2.893 2.893 0 00-.366 1.812 2.906 2.906 0 003.244 2.538 2.899 2.899 0 001.943-1.1 2.89 2.89 0 00.59-2.15 2.905 2.905 0 00-.562-1.396 4.516 4.516 0 01-.542-.641.807.807 0 01.19-1.128.805.805 0 011.126.19c.061.085.122.163.19.24a.846.846 0 01.155.14c.028.034.05.067.077.1.474.429 1.08.692 1.731.74 1.605.12 3-1.09 3.118-2.693a2.913 2.913 0 00-2.681-3.124 2.884 2.884 0 00-1.739.423.804.804 0 01-.9.085.82.82 0 01-.324-1.107c.234-.425.359-.905.359-1.396a2.92 2.92 0 00-2.914-2.918A2.919 2.919 0 009.15 7.04c0 .576.168 1.13.485 1.608.016.024.03.053.043.077a4.52 4.52 0 011.379 3.25c0 1.426-.66 2.707-1.689 3.54v-.008zm-2.843-6.45a2.914 2.914 0 00-2.906 2.918c0 1.61 1.3 2.92 2.906 2.92a2.92 2.92 0 000-5.838z"
            />
        </svg>
        <svg
            v-else-if="avatarConfig?.variant === 'aihubmix'"
            fill="currentColor"
            fill-rule="evenodd"
            viewBox="0 0 24 24"
            :style="{ transform: `scale(${avatarConfig.multiple})` }"
        >
            <path
                clip-rule="evenodd"
                d="M10.853 6.285c.141-.972.455-2.221.942-3.747l.205-.63.206.63c.486 1.526.8 2.775.942 3.748.108.713.108 1.62 0 2.72-.109 1.105-.109 2.019 0 2.741a4.218 4.218 0 001.452 2.635 4.224 4.224 0 002.855 1.07c1.2 0 2.224-.423 3.074-1.268.846-.845 1.273-1.865 1.282-3.06.005-1.058.114-2.225.326-3.5.104-.637.21-1.17.319-1.6l.142-.581.255.538A11.88 11.88 0 0124 10.883v.24c0 1.63-.314 3.186-.942 4.669a12.017 12.017 0 01-6.39 6.39 11.848 11.848 0 01-4.668.942c-1.629 0-3.185-.314-4.668-.942a12.016 12.016 0 01-6.39-6.39A11.848 11.848 0 010 11.124v-.241A11.881 11.881 0 011.148 5.98l.255-.538.141.58c.11.43.215.964.32 1.601.212 1.275.32 2.442.325 3.5.01 1.195.437 2.215 1.282 3.06.85.845 1.875 1.268 3.075 1.268a4.225 4.225 0 002.854-1.07 4.218 4.218 0 001.453-2.635c.108-.722.108-1.636 0-2.741-.109-1.1-.109-2.007 0-2.72zM12 20.936a9.651 9.651 0 004.661-1.176 9.643 9.643 0 002.677-2.113c.095-.107-.017-.27-.154-.232a6.574 6.574 0 01-1.73.227 6.402 6.402 0 01-3.293-.893c-.82-.478-1.5-1.099-2.04-1.862a.149.149 0 00-.242 0 6.427 6.427 0 01-2.04 1.862 6.402 6.402 0 01-3.293.893 6.574 6.574 0 01-1.73-.227c-.137-.037-.248.125-.154.232a9.643 9.643 0 002.677 2.113A9.651 9.651 0 0012 20.935z"
            />
        </svg>
        <svg
            v-else-if="avatarConfig?.variant === 'stepfun'"
            fill="currentColor"
            fill-rule="evenodd"
            viewBox="0 0 24 24"
            :style="{ transform: `scale(${avatarConfig.multiple})` }"
        >
            <path
                d="M1 23h6.335v-6.337H1V23zM8.832 23h6.336v-6.337H8.832V23zM8.832 15.17h6.336V8.835H8.832v6.337zM8.832 7.342h6.336V1.005H8.832v6.337zM16.665 7.337H23V1h-6.335v6.337z"
            />
        </svg>
        <component
            :is="avatarComponent"
            v-else-if="avatarComponent && avatarConfig"
            :style="{
                height: `${size}px`,
                transform: `scale(${avatarConfig.multiple})`,
                width: `${size}px`,
            }"
        />
        <CodeFlowLogo v-else-if="isCodeFlow" :size="size" />
        <span v-else>{{ value.slice(0, 1).toLocaleUpperCase() }}</span>
    </span>

    <span
        v-else-if="kind === 'provider-combine'"
        class="inline-flex min-w-0 flex-none items-center justify-start text-[var(--color-text-primary)]"
        :style="{ gap: `${combineSize / 3}px`, height: `${size * 1.5}px` }"
        aria-hidden="true"
    >
        <template v-if="providerConfig?.combine.kind === 'bedrock'">
            <component
                :is="iconComponents['aws-color']"
                class="special-square"
                :style="{ height: `${combineSize * 1.2}px`, width: `${combineSize * 1.2}px` }"
            />
            <span
                class="block h-[1em] w-px flex-none bg-divider"
                :style="{ margin: `0 ${combineSize / 6}px` }"
            />
            <LobeBrandCombine brand="bedrock" :size="combineSize" />
        </template>
        <template v-else-if="providerConfig?.combine.kind === 'google'">
            <component
                :is="specialAsset"
                :style="{ height: `${combineSize * 0.95}px`, width: 'auto' }"
            />
            <span
                class="block h-[1em] w-px flex-none bg-divider"
                :style="{ margin: `0 ${combineSize / 6}px` }"
            />
            <LobeBrandCombine brand="gemini" :size="combineSize" />
        </template>
        <template v-else-if="providerConfig?.combine.kind === 'azure'">
            <LobeBrandCombine brand="azure" :size="combineSize * 0.92" />
            <span
                class="block h-[1em] w-px flex-none bg-divider"
                :style="{ margin: `0 ${combineSize / 6}px` }"
            />
            <LobeBrandCombine brand="openai" :size="combineSize" />
        </template>
        <template v-else-if="providerConfig?.combine.kind === 'anthropic'">
            <component
                :is="iconComponents['anthropic-text']"
                :style="{ height: `${combineSize * 0.75}px`, width: 'auto' }"
            />
            <span
                class="block h-[1em] w-px flex-none bg-divider"
                :style="{ margin: `0 ${combineSize / 6}px` }"
            />
            <LobeBrandCombine brand="claude" :size="combineSize" />
        </template>
        <template v-else-if="providerConfig?.combine.kind === 'qwen'">
            <LobeBrandCombine brand="alibabacloud" :size="combineSize" />
            <span
                class="block h-[1em] w-px flex-none bg-divider"
                :style="{ margin: `0 ${combineSize / 6}px` }"
            />
            <LobeBrandCombine brand="qwen" :size="combineSize * 0.9" />
        </template>
        <template v-else-if="providerConfig?.combine.kind === 'wenxin'">
            <LobeBrandCombine brand="baiducloud" :size="combineSize * 0.9" />
            <span
                class="block h-[1em] w-px flex-none bg-divider"
                :style="{ margin: `0 ${combineSize / 6}px` }"
            />
            <LobeBrandCombine brand="wenxin" extra="千帆" :size="combineSize" />
        </template>
        <template v-else-if="providerConfig?.combine.kind === 'cloudflare'">
            <LobeBrandCombine brand="cloudflare" :size="combineSize * 1.1" />
            <span
                class="block h-[1em] w-px flex-none bg-divider"
                :style="{ margin: `0 ${combineSize / 6}px` }"
            />
            <LobeBrandCombine brand="workersai" :size="combineSize * 0.9" />
        </template>
        <template v-else-if="providerConfig?.combine.kind === 'v0'">
            <LobeBrandCombine brand="vercel" :size="combineSize * 0.85" />
            <span
                class="block h-[1em] w-px flex-none bg-divider"
                :style="{ margin: `0 ${combineSize / 6}px` }"
            />
            <component
                :is="specialAsset"
                class="special-square"
                :style="{ height: `${combineSize * 1.1}px`, width: `${combineSize * 1.1}px` }"
            />
        </template>
        <LobeBrandCombine
            v-else-if="providerConfig?.combine.kind === 'ollamacloud'"
            brand="ollama"
            :extra-font-size="size * 0.78"
            :extra-margin-left="size * 0.2"
            extra="Cloud"
            :size="size * 1.16"
        />
        <CodeFlowLogo v-else-if="isCodeFlow" :size="combineSize" />
        <LobeBrandCombine v-else :brand="combineBrand" :size="combineSize" />
    </span>

    <span
        v-else-if="kind === 'provider-wordmark'"
        class="lobe-provider-wordmark"
        :style="{
            color: 'var(--color-text-primary)',
            gap: `${Math.max(4, Math.round(size * 0.214))}px`,
        }"
        aria-hidden="true"
    >
        <CodeFlowLogo v-if="isCodeFlow" :size="size" />
        <LobeBrandCombine v-else :brand="combineBrand" :size="combineSize" />
        <span
            v-if="isCodeFlow"
            class="font-semibold"
            :style="{ fontSize: `${Math.round(size * 0.536)}px`, letterSpacing: '-0.01em' }"
            >CodeFlow</span
        >
    </span>

    <span
        v-else
        class="model-icon inline-flex flex-none items-center justify-center overflow-hidden rounded-full"
        :class="{
            'black-background': modelBackground === '#000',
            fallback: !modelMapping,
            'white-background': modelBackground === '#fff',
        }"
        :style="modelAvatarStyle"
        aria-hidden="true"
    >
        <svg
            v-if="modelMapping?.slug === 'aihubmix'"
            fill="currentColor"
            fill-rule="evenodd"
            viewBox="0 0 24 24"
            :style="{ transform: `scale(${modelAvatar?.multiple})` }"
        >
            <path
                clip-rule="evenodd"
                d="M10.853 6.285c.141-.972.455-2.221.942-3.747l.205-.63.206.63c.486 1.526.8 2.775.942 3.748.108.713.108 1.62 0 2.72-.109 1.105-.109 2.019 0 2.741a4.218 4.218 0 001.452 2.635 4.224 4.224 0 002.855 1.07c1.2 0 2.224-.423 3.074-1.268.846-.845 1.273-1.865 1.282-3.06.005-1.058.114-2.225.326-3.5.104-.637.21-1.17.319-1.6l.142-.581.255.538A11.88 11.88 0 0124 10.883v.24c0 1.63-.314 3.186-.942 4.669a12.017 12.017 0 01-6.39 6.39 11.848 11.848 0 01-4.668.942c-1.629 0-3.185-.314-4.668-.942a12.016 12.016 0 01-6.39-6.39A11.848 11.848 0 010 11.124v-.241A11.881 11.881 0 011.148 5.98l.255-.538.141.58c.11.43.215.964.32 1.601.212 1.275.32 2.442.325 3.5.01 1.195.437 2.215 1.282 3.06.85.845 1.875 1.268 3.075 1.268a4.225 4.225 0 002.854-1.07 4.218 4.218 0 001.453-2.635c.108-.722.108-1.636 0-2.741-.109-1.1-.109-2.007 0-2.72zM12 20.936a9.651 9.651 0 004.661-1.176 9.643 9.643 0 002.677-2.113c.095-.107-.017-.27-.154-.232a6.574 6.574 0 01-1.73.227 6.402 6.402 0 01-3.293-.893c-.82-.478-1.5-1.099-2.04-1.862a.149.149 0 00-.242 0 6.427 6.427 0 01-2.04 1.862 6.402 6.402 0 01-3.293.893 6.574 6.574 0 01-1.73-.227c-.137-.037-.248.125-.154.232a9.643 9.643 0 002.677 2.113A9.651 9.651 0 0012 20.935z"
                fill-rule="evenodd"
            />
        </svg>
        <svg
            v-else-if="modelMapping?.slug === 'lg'"
            fill="currentColor"
            fill-rule="evenodd"
            viewBox="0 0 24 24"
            :style="{ transform: `scale(${modelAvatar?.multiple})` }"
        >
            <path
                d="M19.167 19.18a10.082 10.082 0 002.97-7.169v-.549l-.498.003h-6.68v1.12h6.038l-.002.034a9.038 9.038 0 01-8.993 8.41 8.96 8.96 0 01-6.375-2.642 8.962 8.962 0 01-2.64-6.376c0-2.406.939-4.67 2.64-6.373A8.961 8.961 0 0112 2.998l.572.007V1.882l-.57-.007A10.15 10.15 0 001.864 12.011c0 2.708 1.055 5.253 2.97 7.17A10.079 10.079 0 0012 22.15a10.078 10.078 0 007.171-2.97m-6.6-2.942V6.656h-1.14v10.705h3.529V16.24H12.57zM9.703 8.183a1.533 1.533 0 10-3.066-.01 1.533 1.533 0 003.066.01z"
            />
        </svg>
        <svg
            v-else-if="modelMapping?.slug === 'nanobanana'"
            viewBox="0 0 24 24"
            :style="{ transform: `scale(${modelAvatar?.multiple})` }"
        >
            <path
                d="M12.453 1.026c.826-.118 1.574.17 2.207.684.625.508 1.157 1.25 1.596 2.102.797 1.548 1.332 3.555 1.535 5.487a3.689 3.689 0 013.263 1.107l1.634 1.704c.645.674.23 1.89-.775 1.89H19.88l.002.088v5.664l-.014.237c-.028.234-.1.457-.228.647-.177.263-.445.44-.769.485-.613.087-1.256-.302-1.815-.942l-.002-.002-1.387-1.602c-1.57 1.96-4.028 3.442-6.387 4.08-2.409.65-4.976.471-6.262-1.34H2.49c-.823 0-1.49-.668-1.491-1.49l.008-.153A1.492 1.492 0 012.27 18.35c.203-1.603 1.343-2.938 2.804-3.625l.326-.141A6.95 6.95 0 006.554 14H3.587c-1.004 0-1.42-1.218-.775-1.89l1.633-1.704a3.68 3.68 0 015.105-.241 8.88 8.88 0 00.4-1.615c.099-.696.112-1.431.11-2.532 0-.88-.037-2.013.215-2.952.13-.48.342-.946.702-1.319.366-.38.855-.631 1.476-.72z"
                stroke="#451D1C"
            />
            <path
                d="M1.5 19.824c0-.548.444-.992.991-.992h.744a.991.991 0 010 1.983H2.49a.991.991 0 01-.991-.991z"
                fill="#F3AD61"
            />
            <path
                d="M14.837 13.5h7.076c.522 0 .784-.657.413-1.044l-1.634-1.704a3.183 3.183 0 00-4.636 0l-1.633 1.704c-.37.385-.107 1.044.414 1.044zM3.587 13.5h7.076c.521 0 .784-.659.414-1.044l-1.635-1.704a3.183 3.183 0 00-4.636 0l-1.633 1.704c-.37.385-.107 1.044.414 1.044z"
                fill="#F9C23C"
            />
            <path
                d="M12.525 1.521c3.69-.53 5.97 8.923 4.309 12.744-1.662 3.82-5.248 4.657-9.053 6.152a3.49 3.49 0 01-1.279.244c-1.443 0-2.227 1.187-2.774-.282-.707-1.9.22-4.031 2.069-4.757 2.014-.79 3.084-2.308 3.89-4.364.82-2.096.877-2.956.873-5.241-.003-1.827-.123-4.195 1.965-4.496z"
                fill="#FEEFC2"
            />
            <path
                d="M16.834 14.264l-7.095-3.257c-.815 1.873-2.29 3.308-4.156 4.043-2.16.848-3.605 3.171-2.422 5.54 2.364 4.727 13.673-.05 13.673-6.325z"
                fill="#FCD53F"
            />
            <path
                clip-rule="evenodd"
                d="M13.68 12.362c.296.094.46.41.365.707-1.486 4.65-5.818 6.798-9.689 6.997a.562.562 0 11-.057-1.124c3.553-.182 7.372-2.138 8.674-6.216a.562.562 0 01.707-.364z"
                fill="#F9C23C"
                fill-rule="evenodd"
            />
            <path
                d="M17.43 19.85l-7.648-8.835h6.753c1.595.08 2.846 1.433 2.846 3.073v5.664c0 .997-.898 1.302-1.95.098z"
                fill="#FFF478"
            />
        </svg>
        <svg
            v-else-if="modelMapping?.slug === 'sora'"
            viewBox="0 0 24 24"
            :style="{ transform: `scale(${modelAvatar?.multiple})` }"
        >
            <path
                d="M8.968 11.147a.408.408 0 110 .816.408.408 0 010-.816z"
                :fill="`url(#${soraGradientId})`"
            />
            <path
                clip-rule="evenodd"
                d="M7.21 8.748c.045-.012.087.003.128.044.195.2.39.398.587.596a.15.15 0 00.061.035l.81.209c.056.014.09.043.102.088.013.046-.002.09-.043.13l-.596.585a.139.139 0 00-.021.03.134.134 0 00-.015.033c-.07.27-.14.54-.208.81-.014.055-.044.09-.09.102-.045.012-.088-.003-.128-.045-.195-.199-.39-.397-.587-.595a.158.158 0 00-.062-.035l-.81-.209c-.056-.014-.09-.044-.103-.09-.011-.044.004-.087.045-.128.2-.194.398-.39.596-.585a.12.12 0 00.022-.03.134.134 0 00.014-.032c.07-.27.14-.54.208-.81.014-.056.044-.09.09-.103z"
                :fill="`url(#${soraGradientId})`"
                fill-rule="evenodd"
            />
            <path
                d="M15.827 9.31a.409.409 0 110 .817.409.409 0 010-.818z"
                :fill="`url(#${soraGradientId})`"
            />
            <path
                clip-rule="evenodd"
                d="M14.071 6.915c.046-.012.09.003.13.044.194.2.388.398.583.596a.155.155 0 00.062.036l.807.21c.056.014.09.045.103.09.012.045-.003.088-.045.128l-.596.583a.168.168 0 00-.036.061l-.21.808c-.014.056-.044.09-.09.103-.045.011-.088-.004-.128-.045-.194-.2-.389-.398-.583-.596a.12.12 0 00-.03-.022.12.12 0 00-.032-.014l-.808-.21c-.056-.014-.09-.044-.102-.09-.012-.045.003-.087.044-.128.2-.194.398-.388.596-.583a.119.119 0 00.022-.03.132.132 0 00.015-.032l.21-.806c.014-.057.043-.09.088-.103z"
                :fill="`url(#${soraGradientId})`"
                fill-rule="evenodd"
            />
            <path
                clip-rule="evenodd"
                d="M8.086.457a6.102 6.102 0 013.046-.415c1.333.153 2.521.72 3.564 1.7a.116.116 0 00.107.029c1.409-.346 2.762-.224 4.062.366l.061.029.155.077c1.357.703 2.33 1.769 2.918 3.197.278.68.418 1.388.421 2.127a5.65 5.65 0 01-.18 1.631.164.164 0 00.04.154 5.98 5.98 0 011.577 2.892c.386 1.901-.008 3.614-1.182 5.14l-.181.22a6.062 6.062 0 01-2.936 1.85.16.16 0 00-.106.103c-.255.736-.512 1.364-.988 1.992-1.199 1.582-2.962 2.462-4.948 2.45-1.583-.007-2.986-.586-4.21-1.736a.142.142 0 00-.14-.031c-.518.167-1.04.191-1.605.185a5.923 5.923 0 01-2.594-.622 6.057 6.057 0 01-2.146-1.781c-.203-.27-.404-.522-.552-.821a7.742 7.742 0 01-.494-1.283 6.108 6.108 0 01-.017-3.065.163.163 0 00.007-.074.112.112 0 00-.036-.063 5.954 5.954 0 01-1.38-2.202 5.193 5.193 0 01-.333-1.59 6.911 6.911 0 01.188-2.13c.45-1.485 1.309-2.65 2.578-3.494.282-.188.549-.334.8-.439a8.21 8.21 0 01.862-.303.128.128 0 00.087-.087 6.014 6.014 0 011.104-2.155C6.315 1.463 7.132.846 8.086.457zm.965 7.647c-1.154-.82-2.73-.413-3.311.875-.301.666-.36 1.368-.178 2.106l.145.586.26.95c.105.533.31 1.02.612 1.462l.03.043c.16.189.335.362.524.518 1.386 1.139 3.275.379 3.652-1.323l.05-.213.012-.08c.06-.4.042-.792-.053-1.175a47.673 47.673 0 00-.546-2.024c-.217-.738-.616-1.313-1.197-1.725zm7.104-1.646c-.862-.802-2.191-.831-3.047-.026-.334.314-.566.736-.697 1.265a3.47 3.47 0 000 1.635l.014.054.055.18c.127.42.245.834.353 1.241.112.423.202.706.27.85.574 1.206 1.82 2.074 3.177 1.522 1.261-.514 1.641-2.01 1.355-3.22-.043-.183-.09-.365-.14-.546a34.426 34.426 0 00-.428-1.573c-.162-.508-.466-.968-.912-1.382z"
                :fill="`url(#${soraGradientId})`"
                fill-rule="evenodd"
            />
            <defs>
                <linearGradient
                    :id="soraGradientId"
                    gradientUnits="userSpaceOnUse"
                    x1="9.145"
                    x2="14.959"
                    y1="0"
                    y2="24.022"
                >
                    <stop stop-color="#fff" />
                    <stop offset="1" stop-color="#6BB6FE" />
                </linearGradient>
            </defs>
        </svg>
        <svg
            v-else-if="modelMapping?.slug === 'stepfun'"
            fill="currentColor"
            fill-rule="evenodd"
            viewBox="0 0 24 24"
            :style="{ transform: `scale(${modelAvatar?.multiple})` }"
        >
            <path
                d="M1 23h6.335v-6.337H1V23zM8.832 23h6.336v-6.337H8.832V23zM8.832 15.17h6.336V8.835H8.832v6.337zM8.832 7.342h6.336V1.005H8.832v6.337zM16.665 7.337H23V1h-6.335v6.337z"
            />
        </svg>
        <component
            :is="modelComponent"
            v-else-if="modelComponent && modelAvatar"
            :style="{
                height: `${size}px`,
                transform: `scale(${modelAvatar.multiple})`,
                width: `${size}px`,
            }"
        />
        <svg
            v-else-if="!modelMapping"
            class="default-model-icon"
            fill="none"
            stroke="currentColor"
            stroke-linecap="round"
            stroke-linejoin="round"
            stroke-width="2"
            viewBox="0 0 24 24"
        >
            <path d="M12 18V5" />
            <path d="M15 13a4.17 4.17 0 0 1-3-4 4.17 4.17 0 0 1-3 4" />
            <path d="M17.598 6.5A3 3 0 1 0 12 5a3 3 0 1 0-5.598 1.5" />
            <path d="M17.997 5.125a4 4 0 0 1 2.526 5.77" />
            <path d="M18 18a4 4 0 0 0 2-7.464" />
            <path d="M19.967 17.483A4 4 0 1 1 12 18a4 4 0 1 1-7.967-.517" />
            <path d="M6 18a4 4 0 0 1-2-7.464" />
            <path d="M6.003 5.125a4 4 0 0 0-2.526 5.77" />
        </svg>
    </span>
</template>

<style scoped>
.lobe-provider-avatar.white-background,
.model-icon.white-background {
    box-shadow: 0 0 0 1px rgb(0 0 0 / 5%) inset;
}

/* The app keys this on `html.dark-mode`; the window carries the theme here. */
.launcher-preview-window.dark-mode .lobe-provider-avatar.black-background,
.launcher-preview-window.dark-mode .model-icon.black-background {
    box-shadow: 0 0 0 1px rgb(255 255 255 / 10%) inset;
}

.lobe-provider-avatar > :deep(svg),
.lobe-provider-avatar > svg {
    display: block;
    flex: none;
    height: 100%;
    width: 100%;
}

.lobe-provider-wordmark {
    display: inline-flex;
    flex: none;
    align-items: center;
    justify-content: flex-start;
}

.special-square {
    display: block;
    flex: none;
    object-fit: contain;
}

.model-icon > :deep(svg),
.model-icon > svg {
    display: block;
    flex: none;
    width: 100%;
    height: 100%;
}

.default-model-icon {
    transform: scale(0.6);
}
</style>
