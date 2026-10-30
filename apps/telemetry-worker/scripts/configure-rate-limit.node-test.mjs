import assert from 'node:assert/strict'
import { test } from 'node:test'

import { configureRateLimit, telemetryRule } from './configure-rate-limit.mjs'

function mockApi(result, status = 200) {
    const calls = []
    const fetcher = async (url, options) => {
        calls.push({ url, ...options })
        const responseStatus = calls.length === 1 ? status : 200
        return new Response(JSON.stringify({ success: responseStatus < 400, result, errors: [] }), {
            status: responseStatus,
        })
    }
    return { calls, fetcher }
}

const credentials = { apiToken: 'test-only-placeholder', zoneId: 'test-zone' }

test('uses top-level rate limits and parameters supported by a Free zone', () => {
    assert.equal(telemetryRule.action_parameters, undefined)
    assert.deepEqual(telemetryRule.ratelimit.characteristics, ['cf.colo.id', 'ip.src'])
    assert.equal(telemetryRule.ratelimit.period, 10)
    assert.equal(telemetryRule.ratelimit.mitigation_timeout, 10)
})

test('creates a missing phase entrypoint with the telemetry rule', async () => {
    const api = mockApi(null, 404)
    assert.equal(await configureRateLimit({ ...credentials, fetcher: api.fetcher }), 'Created')
    assert.equal(api.calls.length, 2)
    assert.ok(api.calls[0].url.endsWith('/phases/http_ratelimit/entrypoint'))
    assert.equal(api.calls[1].method, 'POST')
    assert.deepEqual(JSON.parse(api.calls[1].body).rules, [telemetryRule])
})

test('adds only the target rule without rewriting unrelated rules', async () => {
    const api = mockApi({ id: 'existing', rules: [{ id: 'unrelated', ratelimit: { period: 60 } }] })
    await configureRateLimit({ ...credentials, fetcher: api.fetcher })
    assert.equal(api.calls[1].method, 'POST')
    assert.ok(api.calls[1].url.endsWith('/existing/rules'))
    assert.deepEqual(JSON.parse(api.calls[1].body), telemetryRule)
})

for (const match of ['description', 'ref']) {
    test(`patches an existing telemetry rule by ${match} without a ruleset PUT`, async () => {
        const api = mockApi({
            id: 'existing',
            rules: [{ id: 'telemetry', [match]: telemetryRule[match] }],
        })
        assert.equal(await configureRateLimit({ ...credentials, fetcher: api.fetcher }), 'Updated')
        assert.equal(api.calls.length, 2)
        assert.equal(api.calls[1].method, 'PATCH')
        assert.ok(api.calls[1].url.endsWith('/existing/rules/telemetry'))
        assert.deepEqual(JSON.parse(api.calls[1].body), telemetryRule)
    })
}

test('does not treat authorization failures as a missing entrypoint', async () => {
    const api = mockApi(null, 403)
    await assert.rejects(
        configureRateLimit({ ...credentials, fetcher: api.fetcher }),
        /Cloudflare API request failed/,
    )
    assert.equal(api.calls.length, 1)
})

test('validates credentials before making an API request', async () => {
    const api = mockApi(null)
    await assert.rejects(
        configureRateLimit({ apiToken: '', zoneId: '', fetcher: api.fetcher }),
        /are required/,
    )
    assert.equal(api.calls.length, 0)
})
