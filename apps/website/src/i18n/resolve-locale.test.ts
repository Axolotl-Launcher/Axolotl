import assert from 'node:assert/strict'
import { test } from 'node:test'

import { resolveInitialLocale, resolveLocale } from './resolve-locale.ts'

const supported = ['en-US', 'zh-CN'] as const
const defaultLocale = 'zh-CN'

test('a supported cookie locale wins over the request header', () => {
    assert.equal(
        resolveLocale({
            cookieLocale: 'en-US',
            acceptLanguage: 'zh-CN,zh;q=0.9',
            supported,
            defaultLocale,
        }),
        'en-US',
    )
})

test('an unsupported cookie locale falls back to the remaining signals', () => {
    assert.equal(
        resolveLocale({
            cookieLocale: 'fr-FR',
            acceptLanguage: 'en-GB,en;q=0.8',
            supported,
            defaultLocale,
        }),
        'en-US',
    )
    assert.equal(resolveLocale({ cookieLocale: 'fr-FR', supported, defaultLocale }), defaultLocale)
})

test('the Accept-Language header wins over browser languages', () => {
    assert.equal(
        resolveLocale({
            acceptLanguage: 'zh-CN,zh;q=0.9,en;q=0.8',
            browserLanguages: ['en-US'],
            supported,
            defaultLocale,
        }),
        'zh-CN',
    )
    assert.equal(
        resolveLocale({
            acceptLanguage: 'fr-FR,fr;q=0.9',
            browserLanguages: ['en-US'],
            supported,
            defaultLocale,
        }),
        'en-US',
    )
})

test('regional zh and en tags normalize to the supported locales', () => {
    assert.equal(
        resolveLocale({ acceptLanguage: 'zh-TW,zh;q=0.9', supported, defaultLocale }),
        'zh-CN',
    )
    assert.equal(
        resolveLocale({ acceptLanguage: 'en-GB,en;q=0.9', supported, defaultLocale }),
        'en-US',
    )
    assert.equal(
        resolveLocale({ browserLanguages: ['zh-Hans-CN', 'en-US'], supported, defaultLocale }),
        'zh-CN',
    )
    assert.equal(resolveLocale({ browserLanguages: ['en-CA'], supported, defaultLocale }), 'en-US')
})

test('browser languages are used when no cookie or header resolves a locale', () => {
    assert.equal(
        resolveLocale({ browserLanguages: ['fr-FR', 'en-US'], supported, defaultLocale }),
        'en-US',
    )
    assert.equal(
        resolveLocale({ browserLanguages: ['ja-JP', 'zh-CN'], supported, defaultLocale }),
        'zh-CN',
    )
})

test('empty input falls back to the default locale', () => {
    assert.equal(resolveLocale({ supported, defaultLocale }), defaultLocale)
    assert.equal(
        resolveLocale({
            cookieLocale: null,
            acceptLanguage: null,
            browserLanguages: [],
            supported,
            defaultLocale,
        }),
        defaultLocale,
    )
    assert.equal(
        resolveLocale({
            acceptLanguage: 'de-DE',
            browserLanguages: ['ko-KR'],
            supported,
            defaultLocale,
        }),
        defaultLocale,
    )
})

test('q weights decide the order, not the written order', () => {
    assert.equal(
        resolveLocale({ acceptLanguage: 'en-US;q=0.1,zh-CN;q=0.9', supported, defaultLocale }),
        'zh-CN',
    )
})

test('a tag without q keeps the implicit weight of 1', () => {
    assert.equal(
        resolveLocale({ acceptLanguage: 'en-US,zh-CN;q=0.9', supported, defaultLocale }),
        'en-US',
    )
})

test('q=0 marks a tag as unacceptable', () => {
    assert.equal(
        resolveLocale({ acceptLanguage: 'zh-CN;q=0,en-US;q=0.5', supported, defaultLocale }),
        'en-US',
    )
    assert.equal(
        resolveLocale({
            acceptLanguage: 'zh-CN;q=0,en-US;q=0',
            browserLanguages: ['zh-CN'],
            supported,
            defaultLocale,
        }),
        'zh-CN',
    )
})

test('equal q weights keep the written order', () => {
    assert.equal(
        resolveLocale({ acceptLanguage: 'en-US;q=0.5,zh-CN;q=0.5', supported, defaultLocale }),
        'en-US',
    )
})

test('an unreadable q weight rejects the tag instead of promoting it', () => {
    // A malformed weight used to be read as 1, which let a broken entry outrank
    // one that legitimately asked for 0.9. It now counts as "not acceptable".
    assert.equal(
        resolveLocale({ acceptLanguage: 'zh-CN;q=abc,en-US;q=0.1', supported, defaultLocale }),
        'en-US',
    )
    assert.equal(
        resolveLocale({ acceptLanguage: 'en-US;q=2,zh-CN;q=0.9', supported, defaultLocale }),
        'zh-CN',
    )
    assert.equal(
        resolveLocale({ acceptLanguage: 'en-US;q=-1,zh-CN;q=0.9', supported, defaultLocale }),
        'zh-CN',
    )
    // No q parameter at all still means fully acceptable.
    assert.equal(
        resolveLocale({ acceptLanguage: 'zh-CN,en-US;q=0.9', supported, defaultLocale }),
        'zh-CN',
    )
})

test('the q parameter is matched case-insensitively and tolerates spacing', () => {
    assert.equal(
        resolveLocale({ acceptLanguage: 'en-US;Q=0.1,zh-CN;Q=0.9', supported, defaultLocale }),
        'zh-CN',
    )
    assert.equal(
        resolveLocale({
            acceptLanguage: 'en-US; q = 0.1, zh-CN; q = 0.9',
            supported,
            defaultLocale,
        }),
        'zh-CN',
    )
})

test('a wildcard and repeated tags do not outrank an explicit choice', () => {
    assert.equal(resolveLocale({ acceptLanguage: '*', supported, defaultLocale }), defaultLocale)
    assert.equal(
        resolveLocale({
            acceptLanguage: 'en-US;q=0,en-US;q=0.1,zh-CN;q=0.9',
            supported,
            defaultLocale,
        }),
        'zh-CN',
    )
})

test('an empty cookie or a header without tags falls through to the default', () => {
    assert.equal(
        resolveLocale({ cookieLocale: '   ', acceptLanguage: ',,,', supported, defaultLocale }),
        defaultLocale,
    )
})

test('a regional variant never normalizes to a locale the caller does not support', () => {
    const frenchOnly = ['fr-FR'] as const
    assert.equal(
        resolveLocale({
            acceptLanguage: 'zh-TW',
            supported: frenchOnly,
            defaultLocale: 'fr-FR',
        }),
        'fr-FR',
    )
    assert.equal(
        resolveLocale({
            cookieLocale: 'zh-TW',
            browserLanguages: ['en-GB'],
            supported: frenchOnly,
            defaultLocale: 'fr-FR',
        }),
        'fr-FR',
    )

    // The base locale is still chosen when the site supports it, even though the
    // exact tag is not in the list; an unlisted variant falls through to it.
    const chinese = ['zh-TW', 'zh-CN'] as const
    assert.equal(
        resolveLocale({ acceptLanguage: 'zh-TW', supported: chinese, defaultLocale: 'zh-TW' }),
        'zh-TW',
    )
    assert.equal(
        resolveLocale({ acceptLanguage: 'zh-HK', supported: chinese, defaultLocale: 'zh-TW' }),
        'zh-CN',
    )
})

test('the client only overrides the server when the server had nothing to go on', () => {
    // The server already answered from the header; a browser whose own preferences
    // disagree must not re-render the page in another language after first paint.
    assert.equal(
        resolveInitialLocale({
            acceptLanguage: 'zh-CN',
            browserLanguages: ['en-US'],
            supported,
            defaultLocale,
        }),
        'zh-CN',
    )
    assert.equal(
        resolveInitialLocale({
            cookieLocale: 'zh-CN',
            browserLanguages: ['en-US'],
            supported,
            defaultLocale,
        }),
        'zh-CN',
    )
    // With no cookie and no header the server could only fall back to the default,
    // so the browser preference is the only real signal and may win.
    assert.equal(
        resolveInitialLocale({ browserLanguages: ['en-US'], supported, defaultLocale }),
        'en-US',
    )
})
