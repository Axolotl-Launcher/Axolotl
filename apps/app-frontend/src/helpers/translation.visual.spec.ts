import { expect, it } from 'vitest'

import {
    prepareDescription,
    renderTranslatedDescription,
    validateTranslatedDescription,
} from './translation'

it.each(['bilingual', 'translation-only'] as const)(
    'renders completed paragraphs while pending or invalid paragraphs retain originals in %s mode',
    (mode) => {
        const prepared = prepareDescription('<p>First paragraph</p><p>Second paragraph</p>', 'html')
        const [first, second] = prepared.segments
        const translations = {
            [second.id]: second.text.replace('Second paragraph', 'Translated second'),
        }
        const read = () => {
            const container = document.createElement('div')
            container.innerHTML = renderTranslatedDescription(
                prepared,
                translations,
                mode,
                'weakened',
            )
            return container
        }
        expect(read().textContent).toContain('First paragraph')
        expect(read().textContent).toContain('Translated second')
        expect(read().querySelectorAll('.ax-translation-block')).toHaveLength(
            mode === 'bilingual' ? 1 : 0,
        )
        expect(() => validateTranslatedDescription(prepared, translations)).toThrow()
        translations[first.id] = '<p>Invalid without protected markers</p>'
        expect(read().textContent).toContain('First paragraph')
        expect(read().textContent).toContain('Translated second')
        expect(read().textContent).not.toContain('Invalid without protected markers')
        expect(() => validateTranslatedDescription(prepared, translations)).toThrow()
        translations[first.id] = first.text.replace('First paragraph', 'Translated first')
        expect(read().textContent).toContain('Translated first')
        expect(read().textContent).toContain('Translated second')
        expect(() => validateTranslatedDescription(prepared, translations)).not.toThrow()
    },
)
