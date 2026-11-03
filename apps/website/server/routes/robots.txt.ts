// Served as a Nitro route so the sitemap link follows runtimeConfig.public.siteUrl
// instead of duplicating the default that nuxt.config.ts owns.
export default defineEventHandler((event) => {
    setHeader(event, 'content-type', 'text/plain; charset=utf-8')
    const { siteUrl } = useRuntimeConfig(event).public
    return `User-agent: *
Allow: /

Sitemap: ${siteUrl}/sitemap.xml
`
})
