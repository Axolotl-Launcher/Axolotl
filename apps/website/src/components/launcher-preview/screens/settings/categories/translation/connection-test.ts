/**
 * The translation provider test of the launcher (`translation::test_provider`
 * in packages/app-lib) always sends the same plain-text segment
 * (`id: "connection-test"`) to the selected service and shows what comes back.
 * The preview cannot reach those services, so it answers with the rendering of
 * that sentence in each language the target-language list offers (the
 * `@modrinth/ui` LOCALES codes).
 */
export const CONNECTION_TEST_TEXT = 'Hello from Axolotl Launcher'

const CONNECTION_TEST_TRANSLATIONS: Record<string, string> = {
    'cs-CZ': 'Ahoj z Axolotl Launcher',
    'da-DK': 'Hej fra Axolotl Launcher',
    'de-CH': 'Hallo vom Axolotl Launcher',
    'de-DE': 'Hallo vom Axolotl Launcher',
    'en-US': CONNECTION_TEST_TEXT,
    'es-419': 'Hola desde Axolotl Launcher',
    'es-ES': 'Hola desde Axolotl Launcher',
    'fi-FI': 'Terveisiä Axolotl Launcherista',
    'fil-PH': 'Kumusta mula sa Axolotl Launcher',
    'fr-FR': 'Bonjour depuis Axolotl Launcher',
    'he-IL': 'שלום מ-Axolotl Launcher',
    'hu-HU': 'Üdvözlet az Axolotl Launcherből',
    'id-ID': 'Halo dari Axolotl Launcher',
    'it-IT': 'Ciao da Axolotl Launcher',
    'ja-JP': 'Axolotl Launcher からこんにちは',
    'ko-KR': 'Axolotl Launcher에서 보내는 인사',
    'ms-MY': 'Helo daripada Axolotl Launcher',
    'nl-NL': 'Hallo van Axolotl Launcher',
    'no-NO': 'Hei fra Axolotl Launcher',
    'pl-PL': 'Pozdrowienia z Axolotl Launcher',
    'pt-BR': 'Olá do Axolotl Launcher',
    'pt-PT': 'Olá do Axolotl Launcher',
    'ro-RO': 'Salutări de la Axolotl Launcher',
    'ru-RU': 'Привет от Axolotl Launcher',
    'sr-CS': 'Pozdrav od Axolotl Launcher-a',
    'sv-SE': 'Hej från Axolotl Launcher',
    'th-TH': 'สวัสดีจาก Axolotl Launcher',
    'tr-TR': "Axolotl Launcher'dan merhaba",
    'uk-UA': 'Привіт від Axolotl Launcher',
    'vi-VN': 'Xin chào từ Axolotl Launcher',
    'zh-CN': '来自 Axolotl Launcher 的问候',
    'zh-TW': '來自 Axolotl Launcher 的問候',
}

/**
 * The test sentence in `target` (a locale code, `_` or `-` separated). A bare
 * language (`zh`, `de`) resolves to the first listed regional variant; a
 * language the list does not carry comes back untranslated, as a service
 * answering in the source language would.
 */
export function connectionTestTranslation(target: string): string {
    const normalized = target.trim().replace(/_/g, '-')
    const exact = Object.keys(CONNECTION_TEST_TRANSLATIONS).find(
        (code) => code.toLowerCase() === normalized.toLowerCase(),
    )
    if (exact) return CONNECTION_TEST_TRANSLATIONS[exact]
    const language = normalized.split('-')[0]?.toLowerCase()
    const variant = Object.keys(CONNECTION_TEST_TRANSLATIONS).find(
        (code) => code.split('-')[0].toLowerCase() === language,
    )
    return variant ? CONNECTION_TEST_TRANSLATIONS[variant] : CONNECTION_TEST_TEXT
}
