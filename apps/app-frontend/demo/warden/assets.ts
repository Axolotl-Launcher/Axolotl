/**
 * The egg's files in `assets/warden/`, as the build serves them. Only their URLs are
 * bundled; each file is fetched when the capture's world first needs it.
 */

const glob = (files: Record<string, string>) =>
    new Map(
        Object.entries(files).map(([path, url]) => [path.slice(path.lastIndexOf('/') + 1), url]),
    )

const scene = glob(
    import.meta.glob<string>('../assets/warden/scene/*', {
        eager: true,
        query: '?url',
        import: 'default',
    }),
)
const sounds = glob(
    import.meta.glob<string>('../assets/warden/sounds/*.ogg', {
        eager: true,
        query: '?url',
        import: 'default',
    }),
)
const textures = glob(
    import.meta.glob<string>('../assets/warden/textures/*.png', {
        eager: true,
        query: '?url',
        import: 'default',
    }),
)

function find(files: Map<string, string>, name: string) {
    const url = files.get(name)
    if (!url) throw new Error(`Missing warden asset ${name}`)
    return url
}

/** A file of the baked world, such as `scene.json` or `atlas_mip1.png`. */
export const sceneFile = (name: string) => find(scene, name)
/** A sound file, named as in Minecraft's asset index (`mob.warden.roar_1`). */
export const soundFile = (name: string) => find(sounds, `${name}.ogg`)
/** A texture, by its file name without `.png`. */
export const textureFile = (name: string) => find(textures, `${name}.png`)
