import assert from 'node:assert/strict'
import { test } from 'node:test'

import { checkDeployment } from './check-deployment.mjs'

function probe(error = 'error_reporting_disabled', status = 400) {
    const calls = []
    const fetcher = async (url, options) => {
        calls.push({ url, ...options })
        return url.endsWith('/health')
            ? Response.json({ status: 'ok', schema_version: 1 })
            : Response.json({ error }, { status })
    }
    return { calls, fetcher }
}

test('checks health and disabled error reporting without sending a valid telemetry envelope', async () => {
    const mock = probe()
    await checkDeployment(mock.fetcher)
    assert.equal(mock.calls.length, 2)
    assert.equal(mock.calls[1].method, 'POST')
    assert.deepEqual(JSON.parse(mock.calls[1].body), { events: [{ type: 'error' }] })
    assert.equal(JSON.parse(mock.calls[1].body).batch_id, undefined)
    assert.equal(JSON.parse(mock.calls[1].body).installation_id, undefined)
})

test('fails when a healthy custom domain still serves the legacy schema', async () => {
    await assert.rejects(checkDeployment(probe('invalid_batch').fetcher), /heartbeat-only Worker/)
})

test('fails when the probe is unexpectedly accepted or blocked by WAF', async () => {
    for (const status of [200, 403, 429])
        await assert.rejects(checkDeployment(probe('unexpected', status).fetcher), /not rejected/)
})

test('stops before the POST when the health endpoint fails', async () => {
    let calls = 0
    await assert.rejects(
        checkDeployment(async () => {
            calls++
            return Response.json({ status: 'unavailable' }, { status: 503 })
        }),
        /health endpoint failed/,
    )
    assert.equal(calls, 1)
})
