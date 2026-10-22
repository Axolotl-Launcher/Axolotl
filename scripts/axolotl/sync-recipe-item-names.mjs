import fs from 'node:fs/promises'

const MINECRAFT_ASSETS_VERSION = '13f6f6ec4fae33d9724b16ed1e332657a6c125c8'
const ITEMS_ROOT = new URL(
    '../../apps/app-frontend/src/lab/recipe-generator/assets/items/',
    import.meta.url,
)
const OUTPUT_FILE = new URL(
    '../../apps/app-frontend/src/lab/recipe-generator/assets/vanilla/item-name-index.json',
    import.meta.url,
)

function languageUrl(locale) {
    return `https://cdn.jsdelivr.net/gh/InventivetalentDev/minecraft-assets@${MINECRAFT_ASSETS_VERSION}/assets/minecraft/lang/${locale}.json`
}

async function downloadJson(url) {
    const response = await fetch(url)
    if (!response.ok) throw new Error(`Unable to download ${url}: HTTP ${response.status}`)
    return await response.json()
}

function itemTranslationKey(id, enUs) {
    if (!id.startsWith('minecraft:') || /:\d+$/.test(id)) return undefined
    const path = id.slice('minecraft:'.length)
    const blockKey = `block.minecraft.${path}`
    const itemKey = `item.minecraft.${path}`
    if (typeof enUs[blockKey] === 'string') return blockKey
    if (typeof enUs[itemKey] === 'string') return itemKey
    return undefined
}

const manifestFiles = (await fs.readdir(ITEMS_ROOT)).filter((file) => file.endsWith('.json'))
const readableById = new Map()

for (const file of manifestFiles) {
    const manifest = JSON.parse(await fs.readFile(new URL(file, ITEMS_ROOT), 'utf8'))
    for (const item of manifest.items ?? []) {
        readableById.set(item.id, item.readable)
    }
}

const [enUs, zhCn] = await Promise.all([
    downloadJson(languageUrl('en_us')),
    downloadJson(languageUrl('zh_cn')),
])

const itemNameIndex = { en_us: {}, zh_cn: {} }
for (const [id, readable] of readableById) {
    const key = itemTranslationKey(id, enUs)
    if (!key) continue
    itemNameIndex.en_us[key] = enUs[key] ?? readable
    itemNameIndex.zh_cn[key] = zhCn[key] ?? itemNameIndex.en_us[key]
}

for (const key of Object.keys(enUs)) {
    if (!/^(?:item\.minecraft\.(?:potion|splash_potion|lingering_potion)\.effect\.|enchantment\.|effect\.)/.test(key)) continue
    itemNameIndex.en_us[key] = enUs[key]
    itemNameIndex.zh_cn[key] = zhCn[key] ?? enUs[key]
}

for (const locale of ['en_us', 'zh_cn']) {
    itemNameIndex[locale] = Object.fromEntries(
        Object.entries(itemNameIndex[locale]).sort(([left], [right]) => left.localeCompare(right)),
    )
}

await fs.mkdir(new URL('.', OUTPUT_FILE), { recursive: true })
await fs.writeFile(OUTPUT_FILE, `${JSON.stringify(itemNameIndex, null, 2)}\n`)

console.log(
    `Updated recipe item names from Minecraft assets ${MINECRAFT_ASSETS_VERSION}: ${Object.keys(itemNameIndex.en_us).length} en_us entries and ${Object.keys(itemNameIndex.zh_cn).length} zh_cn entries.`,
)
