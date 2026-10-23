import assert from 'node:assert/strict'
import { test } from 'node:test'

import { resolveLocale } from './resolve-locale.ts'

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

test('a malformed or out-of-range q weight falls back to 1', () => {
    assert.equal(
        resolveLocale({ acceptLanguage: 'zh-CN;q=abc,en-US', supported, defaultLocale }),
        'zh-CN',
    )
    assert.equal(
        resolveLocale({ acceptLanguage: 'en-US;q=2,zh-CN;q=0.9', supported, defaultLocale }),
        'en-US',
    )
    assert.equal(
        resolveLocale({ acceptLanguage: 'en-US;q=-1,zh-CN;q=0.9', supported, defaultLocale }),
        'en-US',
    )
})
