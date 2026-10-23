import { defineMessages } from '@modrinth/ui/src/composables/i18n.ts'

import { registerPreviewMessages } from '../../../lib/i18n'

/*
 * App strings the settings modal needs that the generated groups in ../messages.ts
 * do not carry (the icon picker, the Java argument presets and the GC decision
 * path). Ids, English and zh-CN are the app's own
 * (apps/app-frontend/src/locales/{en-US,zh-CN}/index.json).
 */

export const iconPickerMessages = defineMessages({
    title: {
        id: 'app.instance.icon-picker.title',
        defaultMessage: 'Create an instance icon',
    },
    description: {
        id: 'app.instance.icon-picker.description',
        defaultMessage: 'Combine a background and a Minecraft element, or upload your own image.',
    },
    background: {
        id: 'app.instance.icon-picker.background',
        defaultMessage: 'Background',
    },
    surpriseMe: {
        id: 'app.instance.icon-picker.surprise-me',
        defaultMessage: 'Surprise me',
    },
    upload: {
        id: 'app.instance.icon-picker.upload',
        defaultMessage: 'Upload image',
    },
    useIcon: {
        id: 'app.instance.icon-picker.use-icon',
        defaultMessage: 'Use this icon',
    },
    saving: {
        id: 'app.instance.icon-picker.saving',
        defaultMessage: 'Saving...',
    },
    loadError: {
        id: 'app.instance.icon-picker.load-error',
        defaultMessage: 'Failed to load the bundled icon.',
    },
    groupOriginal: {
        id: 'app.instance.icon-picker.group.original',
        defaultMessage: 'Original icons',
    },
    groupModrinth3d: {
        id: 'app.instance.icon-picker.group.modrinth-3d',
        defaultMessage: 'Modrinth 3D icons',
    },
})

export const iconBackgroundMessages = defineMessages({
    transparent: {
        id: 'app.instance.icon-picker.background.transparent',
        defaultMessage: 'Transparent',
    },
    grass: {
        id: 'app.instance.icon-picker.background.grass',
        defaultMessage: 'Grass',
    },
    ocean: {
        id: 'app.instance.icon-picker.background.ocean',
        defaultMessage: 'Ocean',
    },
    amethyst: {
        id: 'app.instance.icon-picker.background.amethyst',
        defaultMessage: 'Amethyst',
    },
    sunset: {
        id: 'app.instance.icon-picker.background.sunset',
        defaultMessage: 'Sunset',
    },
    cherry: {
        id: 'app.instance.icon-picker.background.cherry',
        defaultMessage: 'Cherry',
    },
    nether: {
        id: 'app.instance.icon-picker.background.nether',
        defaultMessage: 'Nether',
    },
    slime: {
        id: 'app.instance.icon-picker.background.slime',
        defaultMessage: 'Slime',
    },
    deepDark: {
        id: 'app.instance.icon-picker.background.deep-dark',
        defaultMessage: 'Deep Dark',
    },
    stone: {
        id: 'app.instance.icon-picker.background.stone',
        defaultMessage: 'Stone',
    },
    midnight: {
        id: 'app.instance.icon-picker.background.midnight',
        defaultMessage: 'Midnight',
    },
})

export const javaPresetMessages = defineMessages({
    gcGroupTitle: {
        id: 'app.java-arguments.presets.gc-group-title',
        defaultMessage: 'Memory recycling strategy (GC)',
    },
    authGroupTitle: {
        id: 'app.java-arguments.presets.auth-group-title',
        defaultMessage: 'Authentication service',
    },
    authMirrorTitle: {
        id: 'app.java-arguments.presets.auth-mirror.title',
        defaultMessage: 'Authentication service mirror',
    },
    authMirrorDescription: {
        id: 'app.java-arguments.presets.auth-mirror.description',
        defaultMessage:
            'HTTP forwarding for the Mojang authentication servers hosted by Fallen-Breath.',
    },
    gcAutoTitle: {
        id: 'app.java-arguments.presets.gc.auto.title',
        defaultMessage: 'Auto',
    },
    gcAutoDescription: {
        id: 'app.java-arguments.presets.gc.auto.description',
        defaultMessage: 'Automatically select the best GC strategy for your system',
    },
    gcG1gcMojangTitle: {
        id: 'app.java-arguments.presets.gc.g1gc-mojang.title',
        defaultMessage: 'Mojang G1GC',
    },
    gcG1gcMojangDescription: {
        id: 'app.java-arguments.presets.gc.g1gc-mojang.description',
        defaultMessage: 'G1GC tuning from the official Minecraft launcher',
    },
    gcPclTitle: {
        id: 'app.java-arguments.presets.gc.g1gc-pcl.title',
        defaultMessage: 'PCL',
    },
    gcPclDescription: {
        id: 'app.java-arguments.presets.gc.g1gc-pcl.description',
        defaultMessage: 'Shenandoah (adaptive) tuning used by the PCL launcher',
    },
    gcShenandoahTitle: {
        id: 'app.java-arguments.presets.gc.shenandoah.title',
        defaultMessage: 'Shenandoah',
    },
    gcShenandoahDescription: {
        id: 'app.java-arguments.presets.gc.shenandoah.description',
        defaultMessage: 'Low-pause adaptive Shenandoah with large pages (if supported)',
    },
    gcZgcTitle: {
        id: 'app.java-arguments.presets.gc.zgc.title',
        defaultMessage: 'ZGC',
    },
    gcZgcDescription: {
        id: 'app.java-arguments.presets.gc.zgc.description',
        defaultMessage: 'Ultra-low latency GC for high-end systems (Java 15+)',
    },
})

export const gcReasonMessages = defineMessages({
    javaVersionUnknown: {
        id: 'app.java-arguments.gc.reason.java-version-unknown',
        defaultMessage: 'Java version unknown',
    },
    javaTooOld: {
        id: 'app.java-arguments.gc.reason.java-too-old',
        defaultMessage: 'Java too old; Shenandoah/ZGC unreliable',
    },
    insufficientMemory: {
        id: 'app.java-arguments.gc.reason.insufficient-memory',
        defaultMessage: 'Insufficient memory ({allocated}GB < 4GB)',
    },
    insufficientCpu: {
        id: 'app.java-arguments.gc.reason.insufficient-cpu',
        defaultMessage: 'Insufficient CPU ({cores} cores / {threads} threads)',
    },
    largeModpackLowMemory: {
        id: 'app.java-arguments.gc.reason.large-modpack-low-memory',
        defaultMessage: 'Large modpack ({mods} mods) but insufficient memory ({allocated}GB < 8GB)',
    },
    lightweightInstance: {
        id: 'app.java-arguments.gc.reason.lightweight-instance',
        defaultMessage: 'Lightweight instance ({loader}, {mods} mods)',
    },
    heavyInstance: {
        id: 'app.java-arguments.gc.reason.heavy-instance',
        defaultMessage: 'Heavy instance ({loader}, {mods} mods)',
    },
    lowResources: {
        id: 'app.java-arguments.gc.reason.low-resources',
        defaultMessage: 'Low resources ({memory}GB, {cores} cores)',
    },
    mediumResources: {
        id: 'app.java-arguments.gc.reason.medium-resources',
        defaultMessage: 'Medium resources ({memory}GB, {cores} cores)',
    },
    highResources: {
        id: 'app.java-arguments.gc.reason.high-resources',
        defaultMessage: 'High resources ({memory}GB, {cores} cores)',
    },
    zgcNonGenerational: {
        id: 'app.java-arguments.gc.reason.zgc-non-generational',
        defaultMessage: 'Java < 21; non-generational ZGC performs poorly',
    },
    ampleMemoryHighCores: {
        id: 'app.java-arguments.gc.reason.ample-memory-high-cores',
        defaultMessage: 'Ample memory and high CPU core count',
    },
    belowZgcRecommendation: {
        id: 'app.java-arguments.gc.reason.below-zgc-recommendation',
        defaultMessage: 'Resources below ZGC recommendation',
    },
})

registerPreviewMessages('zh-CN', {
    'app.instance.icon-picker.title': '创建实例图标',
    'app.instance.icon-picker.description': '组合背景和 Minecraft 元素，或上传自己的图片。',
    'app.instance.icon-picker.background': '背景',
    'app.instance.icon-picker.surprise-me': '随机生成',
    'app.instance.icon-picker.upload': '上传图片',
    'app.instance.icon-picker.use-icon': '使用此图标',
    'app.instance.icon-picker.saving': '保存中……',
    'app.instance.icon-picker.load-error': '无法加载内置图标。',
    'app.instance.icon-picker.group.original': '原版图标',
    'app.instance.icon-picker.group.modrinth-3d': 'Modrinth 3D 图标',
    'app.instance.icon-picker.background.transparent': '透明',
    'app.instance.icon-picker.background.grass': '草地',
    'app.instance.icon-picker.background.ocean': '海洋',
    'app.instance.icon-picker.background.amethyst': '紫水晶',
    'app.instance.icon-picker.background.sunset': '日落',
    'app.instance.icon-picker.background.cherry': '樱花',
    'app.instance.icon-picker.background.nether': '下界',
    'app.instance.icon-picker.background.slime': '史莱姆',
    'app.instance.icon-picker.background.deep-dark': '深暗之域',
    'app.instance.icon-picker.background.stone': '石头',
    'app.instance.icon-picker.background.midnight': '午夜',
    'app.java-arguments.presets.gc-group-title': '内存回收策略 (GC)',
    'app.java-arguments.presets.auth-group-title': 'Mojang认证代理',
    'app.java-arguments.presets.auth-mirror.title': '镜像认证服务',
    'app.java-arguments.presets.auth-mirror.description':
        '来自 Fallen-Breath 搭建的 Mojang 认证服务器 HTTP 转发。',
    'app.java-arguments.presets.gc.auto.title': '自动GC',
    'app.java-arguments.presets.gc.auto.description': '自动为你的系统选择最佳 GC 策略',
    'app.java-arguments.presets.gc.g1gc-mojang.title': 'Mojang G1GC',
    'app.java-arguments.presets.gc.g1gc-mojang.description': '接近官方的 G1GC 调优',
    'app.java-arguments.presets.gc.g1gc-pcl.title': 'PCL',
    'app.java-arguments.presets.gc.g1gc-pcl.description':
        'PCL 启动器使用的 Shenandoah（adaptive）调优',
    'app.java-arguments.presets.gc.shenandoah.title': 'Shenandoah',
    'app.java-arguments.presets.gc.shenandoah.description':
        '自适应低停顿 Shenandoah，启用大页内存（如支持）',
    'app.java-arguments.presets.gc.zgc.title': 'ZGC',
    'app.java-arguments.presets.gc.zgc.description': '面向高端系统的超低延迟 GC（Java 15+）',
    'app.java-arguments.gc.reason.java-version-unknown': 'Java 版本未知',
    'app.java-arguments.gc.reason.java-too-old': 'Java 太旧，Shenandoah/ZGC 不可靠',
    'app.java-arguments.gc.reason.insufficient-memory': '内存不足 ({allocated}GB < 4GB)',
    'app.java-arguments.gc.reason.insufficient-cpu': 'CPU 资源不足 ({cores}核/{threads}线程)',
    'app.java-arguments.gc.reason.large-modpack-low-memory':
        '大型 ModPack ({mods} mods) 但资源不足 ({allocated}GB < 8GB)',
    'app.java-arguments.gc.reason.lightweight-instance': '轻量实例 ({loader}, {mods} mods)',
    'app.java-arguments.gc.reason.heavy-instance': '重型实例 ({loader}, {mods} mods)',
    'app.java-arguments.gc.reason.low-resources': '资源低 ({memory}GB, {cores}核)',
    'app.java-arguments.gc.reason.medium-resources': '资源中 ({memory}GB, {cores}核)',
    'app.java-arguments.gc.reason.high-resources': '资源高 ({memory}GB, {cores}核)',
    'app.java-arguments.gc.reason.zgc-non-generational': 'Java < 21，ZGC 非分代模式性能不佳',
    'app.java-arguments.gc.reason.ample-memory-high-cores': '内存充足且 CPU 核心数高',
    'app.java-arguments.gc.reason.below-zgc-recommendation': '资源未达到 ZGC 推荐配置',
})
