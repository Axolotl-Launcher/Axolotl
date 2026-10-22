/** Counts cached image payloads without creating or upgrading a database. */
export async function previewCacheUsage(): Promise<number | null> {
    if (!indexedDB.databases) return null
    const databases = await indexedDB.databases()
    let bytes = 0
    for (const { name } of databases) {
        if (!name || (name !== 'head-storage' && !name.startsWith('skin-previews'))) continue
        bytes += await new Promise<number>((resolve, reject) => {
            const request = indexedDB.open(name)
            request.onupgradeneeded = () => request.transaction?.abort()
            request.onerror = () => reject(request.error)
            request.onsuccess = () => {
                const database = request.result
                const stores = Array.from(database.objectStoreNames)
                if (!stores.length) {
                    database.close()
                    resolve(0)
                    return
                }
                const transaction = database.transaction(stores, 'readonly')
                let size = 0
                transaction.oncomplete = () => {
                    database.close()
                    resolve(size)
                }
                transaction.onabort = () => {
                    database.close()
                    reject(transaction.error)
                }
                for (const store of stores) {
                    const cursor = transaction.objectStore(store).openCursor()
                    cursor.onsuccess = () => {
                        const item = cursor.result
                        if (!item) return
                        const value = item.value as { blob?: Blob; forwards?: Blob }
                        const blob = value?.blob ?? value?.forwards
                        if (blob instanceof Blob) size += blob.size
                        item.continue()
                    }
                }
            }
        })
    }
    return bytes
}
