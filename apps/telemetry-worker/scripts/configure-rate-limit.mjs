import { pathToFileURL } from 'node:url'

export const telemetryRule = {
    action: 'block',
    ref: 'axolotl_telemetry_ingress',
    description: 'Axolotl telemetry batch ingress limit',
    enabled: true,
    expression:
        'http.host eq "telemetry.axlmc.org" and http.request.method eq "POST" and http.request.uri.path eq "/v1/batch"',
    ratelimit: {
        characteristics: ['cf.colo.id', 'ip.src'],
        period: 10,
        requests_per_period: 2,
        mitigation_timeout: 10,
    },
}

export async function configureRateLimit({
    apiToken = process.env.CF_API_TOKEN,
    zoneId = process.env.CF_ZONE_ID,
    fetcher = fetch,
} = {}) {
    if (!apiToken || !zoneId) {
        throw new Error('CF_API_TOKEN and CF_ZONE_ID are required')
    }
    const apiBase = `https://api.cloudflare.com/client/v4/zones/${zoneId}/rulesets`
    async function request(path, options = {}, allowMissing = false) {
        const response = await fetcher(`${apiBase}${path}`, {
            ...options,
            headers: {
                Authorization: `Bearer ${apiToken}`,
                'Content-Type': 'application/json',
            },
        })
        if (allowMissing && response.status === 404) return null
        const payload = await response.json()
        if (!response.ok || !payload.success) {
            throw new Error(
                `Cloudflare API request failed: ${JSON.stringify(payload.errors ?? payload)}`,
            )
        }
        return payload.result
    }

    const ruleset = await request('/phases/http_ratelimit/entrypoint', {}, true)
    if (!ruleset) {
        await request('', {
            method: 'POST',
            body: JSON.stringify({
                kind: 'zone',
                name: 'Axolotl rate limits',
                phase: 'http_ratelimit',
                rules: [telemetryRule],
            }),
        })
        return 'Created'
    }

    const existing = ruleset.rules.find(
        (rule) => rule.ref === telemetryRule.ref || rule.description === telemetryRule.description,
    )
    await request(`/${ruleset.id}/rules${existing ? `/${existing.id}` : ''}`, {
        method: existing ? 'PATCH' : 'POST',
        body: JSON.stringify(telemetryRule),
    })
    return existing ? 'Updated' : 'Created'
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
    const result = await configureRateLimit()
    console.log(`${result} the Axolotl telemetry batch rate-limit rule`)
}
