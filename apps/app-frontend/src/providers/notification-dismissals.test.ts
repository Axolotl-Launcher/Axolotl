import type {
    PopupNotification,
    PopupNotificationProgressItem,
    WebNotification,
} from '@modrinth/ui'
import { expect, it } from 'vitest'

import { loadNotificationDismissals } from './notification-dismissals.ts'
import {
    legacyPopupNotificationKey,
    popupNotificationKey,
    webNotificationKey,
} from './notification-keys.ts'

const originalStorageDescriptor = Object.getOwnPropertyDescriptor(globalThis, 'localStorage')

function installMemoryStorage() {
    const values = new Map<string, string>()
    Object.defineProperty(globalThis, 'localStorage', {
        configurable: true,
        value: {
            getItem: (key: string) => values.get(key) ?? null,
            setItem: (key: string, value: string) => values.set(key, value),
        },
    })
    return values
}

function restoreStorage() {
    if (originalStorageDescriptor) {
        Object.defineProperty(globalThis, 'localStorage', originalStorageDescriptor)
    } else {
        delete (globalThis as { localStorage?: Storage }).localStorage
    }
}

function progressItems(...ids: string[]): PopupNotificationProgressItem[] {
    return ids.map((id) => ({ id, title: id, progress: 0, waiting: false }))
}

function popupNotification(
    progressItemsValue?: PopupNotificationProgressItem[],
): PopupNotification {
    return {
        id: 1,
        title: '安装完成',
        text: '实例已安装',
        type: 'success',
        autoCloseMs: null,
        progressItems: progressItemsValue,
    }
}

function readDismissals(keys: Record<string, unknown>, fieldCount: number, legacyPrefixLength = 0) {
    const values = installMemoryStorage()
    try {
        for (const [key, value] of Object.entries(keys)) values.set(key, JSON.stringify(value))
        return loadNotificationDismissals(Object.keys(keys), fieldCount, legacyPrefixLength)
    } finally {
        restoreStorage()
    }
}

it('legacy popup keys dismiss notifications with and without progress items', () => {
    const notification = popupNotification(progressItems('job-1'))
    const dismissed = readDismissals(
        {
            'axolotl:dismissed-popup-notifications-v2': [],
            'axolotl:dismissed-popup-notifications': {
                keys: [legacyPopupNotificationKey(notification)],
            },
        },
        7,
        3,
    )

    expect(dismissed.legacyKeys.has(legacyPopupNotificationKey(popupNotification()))).toBeTruthy()
    expect(dismissed.legacyKeys.has(legacyPopupNotificationKey(notification))).toBeTruthy()
})

it('six-field popup v2 keys migrate to seven fields without hiding another progress set', () => {
    const notification = popupNotification()
    const dismissed = readDismissals(
        {
            'axolotl:dismissed-popup-notifications-v2': {
                keys: [JSON.stringify(['安装完成', '实例已安装', 'success', '', '', ''])],
            },
            'axolotl:dismissed-popup-notifications': {},
        },
        7,
        3,
    )

    expect(dismissed.keys.has(popupNotificationKey(notification))).toBeTruthy()
    expect(
        !dismissed.keys.has(popupNotificationKey(popupNotification(progressItems('job-1')))),
    ).toBeTruthy()
})

it('seven-field popup keys sort progress ids and distinguish different sets', () => {
    const dismissed = readDismissals(
        {
            'axolotl:dismissed-popup-notifications-v2': {
                keys: [
                    JSON.stringify([
                        '安装完成',
                        '实例已安装',
                        'success',
                        '',
                        '',
                        '',
                        'job-1|job-2',
                    ]),
                ],
            },
        },
        7,
    )

    expect(
        dismissed.keys.has(
            popupNotificationKey(popupNotification(progressItems('job-2', 'job-1'))),
        ),
    ).toBeTruthy()
    expect(
        !dismissed.keys.has(popupNotificationKey(popupNotification(progressItems('job-3')))),
    ).toBeTruthy()
})

it('clearedAt and web notification records remain compatible', () => {
    const webNotification: WebNotification = {
        id: 1,
        title: '旧标题',
        text: '旧内容',
        type: 'warning',
    }
    const dismissed = readDismissals(
        {
            'axolotl:dismissed-web-notifications-v2': {
                clearedAt: Date.now() + 1000,
                keys: [webNotificationKey(webNotification)],
            },
            'axolotl:dismissed-web-notifications': {
                keys: [JSON.stringify(['legacy', 'content', 'error', 'E_TEST'])],
            },
        },
        4,
    )

    expect(dismissed.clearedAt! > Date.now()).toBe(true)
    expect(dismissed.keys.has(webNotificationKey(webNotification))).toBeTruthy()
    expect(
        dismissed.keys.has(JSON.stringify(['legacy', 'content', 'error', 'E_TEST'])),
    ).toBeTruthy()
})
