import { fileURLToPath } from 'node:url'

import svgLoader from 'vite-svg-loader'

const UI_PUBLIC_ENTRY = fileURLToPath(new URL('../../packages/ui/index.ts', import.meta.url))
const SITE_URL = 'https://axlmc.org'

export default defineNuxtConfig({
    srcDir: 'src/',
    devtools: { enabled: false },
    app: {
        head: {
            htmlAttrs: {
                class: 'accent-pink dark-mode',
                lang: 'zh-CN',
            },
            title: 'Axolotl Launcher - 免费开源的 Minecraft 启动器',
            // 在 body 渲染前同步应用已保存的主题偏好，避免浅色用户首屏闪深色
            script: [
                {
                    key: 'theme-init',
                    innerHTML: `(function(){try{var t=localStorage.getItem('axolotl-theme');var r=t;if(!r||r==='system'){r=(window.matchMedia&&window.matchMedia('(prefers-color-scheme: light)').matches)?'light':'dark'}var d=document.documentElement;d.classList.remove('light-mode','dark-mode','oled-mode');d.classList.add(r==='light'?'light-mode':r==='oled'?'oled-mode':'dark-mode');d.style.colorScheme=r==='light'?'light':'dark'}catch(e){}})()`,
                },
            ],
            link: [
                { rel: 'icon', type: 'image/png', href: '/axolotl.png' },
                { rel: 'apple-touch-icon', type: 'image/png', href: '/axolotl.png' },
            ],
        },
    },
    runtimeConfig: {
        public: {
            siteUrl: SITE_URL,
        },
    },
    experimental: {
        // Keep the app manifest disabled for the static website. Nuxt's runtime
        // composable still contains a server-side `#app-manifest` import, which
        // Vite tries to resolve during dev even when the feature is disabled.
        appManifest: false,
    },
    vite: {
        css: {
            preprocessorOptions: {
                scss: {
                    silenceDeprecations: ['import'],
                },
            },
        },
        resolve: {
            alias: [{ find: /^@modrinth\/ui$/, replacement: UI_PUBLIC_ENTRY }],
            dedupe: ['vue'],
        },
        plugins: [
            svgLoader({
                svgoConfig: {
                    plugins: [
                        {
                            name: 'preset-default',
                            params: {
                                overrides: {
                                    removeViewBox: false,
                                    cleanupIds: { minify: false },
                                },
                            },
                        },
                    ],
                },
            }),
        ],
    },
    css: ['~/assets/styles/tailwind.css'],
    postcss: {
        plugins: {
            tailwindcss: {},
            autoprefixer: {},
        },
    },
    nitro: {
        prerender: {
            crawlLinks: false,
            routes: ['/', '/changelog', '/terms', '/privacy'],
        },
    },
    routeRules: {
        '/': { static: true },
        '/changelog': { static: true },
        '/terms': { static: true },
        '/privacy': { static: true },
    },
    typescript: {
        shim: false,
        strict: true,
        typeCheck: false,
    },
    compatibilityDate: '2025-01-01',
    telemetry: false,
})
