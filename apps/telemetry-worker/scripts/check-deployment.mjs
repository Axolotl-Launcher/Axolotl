import assert from 'node:assert/strict'
import { setTimeout } from 'node:timers/promises'
import { pathToFileURL } from 'node:url'

export async function checkDeployment(fetcher = fetch) {
    const origin = 'https://telemetry.axlmc.org'
    const health = await fetcher(`${origin}/health`, { signal: AbortSignal.timeout(15_000) })
    assert.equal(health.status, 200, 'Worker health endpoint failed')
    const healthBody = await health.json()
    assert.equal(healthBody.status, 'ok')
    assert.equal(healthBody.schema_version, 1)

    // Missing required envelope/event fields guarantees rejection even on the
    // legacy error-capable Worker. Never submit a valid production telemetry batch.
    const response = await fetcher(`${origin}/v1/batch`, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ events: [{ type: 'error' }] }),
        signal: AbortSignal.timeout(15_000),
    })
    assert.equal(response.status, 400, 'Legacy error probe was not rejected')
    assert.equal(
        (await response.json()).error,
        'error_reporting_disabled',
        'Custom domain is not serving the heartbeat-only Worker',
    )
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
    for (let attempt = 1; attempt <= 6; attempt++) {
        try {
            await checkDeployment()
            console.log(
                'Worker health and legacy error rejection verified; no telemetry data submitted',
            )
            break
        } catch (error) {
            if (attempt === 6) throw error
            console.warn(`Deployment check attempt ${attempt} failed: ${error.message}`)
            await setTimeout(15_000)
        }
    }
}
