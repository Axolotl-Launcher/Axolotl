import type { FilePickerProvider, PickedFile } from '@modrinth/ui/src/providers/file-picker.ts'

/**
 * The app's file picker provider (`provideFilePicker` in App.vue) for the
 * preview: the browser's file dialog stands in for the native one. Files stay
 * in the page; there is no native path to hand to a backend.
 */
function choose(accept: string): Promise<File | null> {
    return new Promise((resolve) => {
        const input = document.createElement('input')
        input.type = 'file'
        input.accept = accept
        input.addEventListener('change', () => resolve(input.files?.[0] ?? null), { once: true })
        input.addEventListener('cancel', () => resolve(null), { once: true })
        input.click()
    })
}

async function pickImage(): Promise<PickedFile | null> {
    const file = await choose('image/png,image/jpeg,image/gif,image/webp,image/svg+xml')
    return file ? { file, previewUrl: URL.createObjectURL(file) } : null
}

export function createPreviewFilePicker(): FilePickerProvider {
    return {
        pickImage,
        async pickModpackFile() {
            const file = await choose('.mrpack')
            return file ? { file, previewUrl: '' } : null
        },
    }
}
