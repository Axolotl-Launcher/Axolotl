// Served as a Nitro route so every <loc> follows runtimeConfig.public.siteUrl
// instead of duplicating the default that nuxt.config.ts owns.
const SITEMAP_PATHS = [
    { path: '', lastmod: '2026-10-16' },
    { path: 'changelog', lastmod: '2026-08-10' },
    { path: 'terms', lastmod: '2026-08-02' },
    { path: 'privacy', lastmod: '2026-08-02' },
] as const

export default defineEventHandler((event) => {
    setHeader(event, 'content-type', 'application/xml; charset=utf-8')
    const { siteUrl } = useRuntimeConfig(event).public
    const entries = SITEMAP_PATHS.map(
        ({ path, lastmod }) =>
            `    <url>\n        <loc>${siteUrl}/${path}</loc>\n        <lastmod>${lastmod}</lastmod>\n    </url>\n`,
    ).join('')
    return `<?xml version="1.0" encoding="UTF-8" ?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n${entries}</urlset>\n`
})
