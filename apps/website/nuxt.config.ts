import { fileURLToPath } from 'node:url'

import svgLoader from 'vite-svg-loader'

const UI_PUBLIC_ENTRY = fileURLToPath(new URL('../../packages/ui/index.ts', import.meta.url))
const SITE_URL = 'https://axlmc.org'
// The only defaults for infrastructure addresses. Deployments override each one with
// its NUXT_PUBLIC_* variable; the rest of the source reads runtimeConfig.public.
const GITHUB_URL = 'https://github.com/Axolotl-Launcher/Axolotl'
const UPDATE_SERVER_URL = 'https://update.axlmc.org'
const UPDATE_ARTIFACT_URL = 'https://dist.axlmc.org'
const GITLINK_RELEASES_URL = 'https://www.gitlink.org.cn/xinvxueyuan/Axolotl/releases/download'

export default defineNuxtConfig({
    srcDir: 'src/',
    dir: {
        // Nuxt 4 resolves `dir.public` against the project root, while this site
        // keeps every source directory (including its static assets) under `src/`.
        public: 'src/public',
    },
    app: {
        head: {
            // Theme classes are applied by the pre-hydration script below. Keeping
            // them out of htmlAttrs prevents Nuxt from re-adding a stale class.
            htmlAttrs: {
                class: 'accent-pink',
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
            githubUrl: process.env.NUXT_PUBLIC_GITHUB_URL || GITHUB_URL,
            updateServerUrl: process.env.NUXT_PUBLIC_UPDATE_SERVER_URL || UPDATE_SERVER_URL,
            updateArtifactUrl: process.env.NUXT_PUBLIC_UPDATE_ARTIFACT_URL || UPDATE_ARTIFACT_URL,
            gitlinkReleasesUrl:
                process.env.NUXT_PUBLIC_GITLINK_RELEASES_URL || GITLINK_RELEASES_URL,
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
            // /llms.txt, /robots.txt and /sitemap.xml are server routes, so they need
            // explicit prerender entries to end up as static files in the generated output.
            routes: [
                '/',
                '/changelog',
                '/terms',
                '/privacy',
                '/llms.txt',
                '/robots.txt',
                '/sitemap.xml',
            ],
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
