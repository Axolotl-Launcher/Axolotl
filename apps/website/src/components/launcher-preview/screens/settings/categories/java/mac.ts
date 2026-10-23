import { PREVIEW_APP_DIRECTORY } from '../../state'

/**
 * The preview Mac as the launcher's Java code sees it (packages/app-lib/src/util/jre.rs,
 * api/jre.rs): its memory and CPU, the Java runtimes installed on it, and the part of
 * its disk the Java pages touch (the JDK folders macOS keeps in
 * `/Library/Java/JavaVirtualMachines`, the `java` stub on the PATH, the launcher's own
 * `meta/java_versions` and the user's home). The machine is an Intel MacBook Pro with
 * 16 GB of memory; paths, dates and sizes are example local values.
 */

const GiB = 1024 ** 3

/** `sysinfo` total memory: 16 GB of RAM. */
export const PREVIEW_TOTAL_MEMORY_BYTES = 16 * GiB
/** `navigator.hardwareConcurrency` of the webview: a quad-core CPU with Hyper-Threading. */
export const PREVIEW_CPU_THREADS = 8
/** `std::env::consts::ARCH`. */
export const PREVIEW_ARCH = 'x86_64'
/** `mojang_runtime_platform()` for macOS on x86_64. */
export const MOJANG_RUNTIME_PLATFORM = 'mac-os'

export const HOME = '/Users/steve'
/** `state.directories.java_versions_dir()`. */
export const JAVA_VERSIONS_DIR = `${PREVIEW_APP_DIRECTORY}/meta/java_versions`
/** `JAVA_BIN` on unix. */
export const JAVA_BIN = 'java'

/** What `check_java_at_filepath` learns about a runtime. */
export interface RuntimeInfo {
    major: number
    /**
     * The runtime's `release` file (`OS_ARCH`, `IMPLEMENTOR`), read without starting a
     * JVM. Null when the executable has none next to it, so the JVM is run instead.
     */
    release: { arch: string; implementor: string | null } | null
    /** `os.arch` the started JVM reports (used when there is no release file). */
    jvmArch: string
}

export interface FsNode {
    name: string
    kind: 'dir' | 'file'
    modified: string
    size: number
    children?: FsNode[]
    /** Hidden by the macOS open panel (dot files, `~/Library`, the Unix folders). */
    hidden?: boolean
    /** A bundle Finder shows as one item (apps); the open panel does not enter it. */
    package?: boolean
    /** Set on a runtime's `java` executable. */
    runtime?: RuntimeInfo
}

function dir(
    name: string,
    modified: string,
    children: FsNode[] = [],
    extra: Partial<FsNode> = {},
): FsNode {
    return { name, kind: 'dir', modified, size: 0, children, ...extra }
}

function file(name: string, modified: string, size: number, extra: Partial<FsNode> = {}): FsNode {
    return { name, kind: 'file', modified, size, ...extra }
}

/** The tools of a JDK's `bin` folder; `java` carries the runtime. */
const JDK_TOOLS: [string, number][] = [
    ['jar', 50_528],
    ['jarsigner', 50_544],
    ['java', 50_496],
    ['javac', 50_528],
    ['javadoc', 50_544],
    ['javap', 50_528],
    ['jcmd', 50_528],
    ['jconsole', 50_544],
    ['jdb', 50_512],
    ['jdeps', 50_528],
    ['jfr', 50_512],
    ['jinfo', 50_528],
    ['jlink', 50_528],
    ['jmap', 50_512],
    ['jmod', 50_512],
    ['jpackage', 50_544],
    ['jps', 50_512],
    ['jshell', 50_528],
    ['jstack', 50_528],
    ['jstat', 50_512],
    ['keytool', 50_544],
    ['rmiregistry', 50_560],
    ['serialver', 50_544],
]

const JRE_TOOLS: [string, number][] = [
    ['java', 50_496],
    ['jfr', 50_512],
    ['jrunscript', 50_544],
    ['keytool', 50_544],
    ['rmiregistry', 50_560],
]

/** `Contents/Home` of a macOS runtime bundle. */
function runtimeHome(modified: string, runtime: RuntimeInfo, jre = false): FsNode {
    const tools = jre ? JRE_TOOLS : JDK_TOOLS
    return dir('Home', modified, [
        dir(
            'bin',
            modified,
            tools.map(([name, size]) =>
                file(name, modified, size, name === 'java' ? { runtime } : {}),
            ),
        ),
        dir('conf', modified),
        ...(jre ? [] : [dir('include', modified), dir('jmods', modified)]),
        dir('legal', modified),
        dir('lib', modified),
        ...(jre ? [] : [dir('man', modified)]),
        ...(runtime.release ? [file('release', modified, 1_204)] : []),
    ])
}

/** A `.jdk` bundle: `Contents/{Home, Info.plist, MacOS}`. */
function jdkBundle(name: string, modified: string, runtime: RuntimeInfo): FsNode {
    return dir(name, modified, [
        dir('Contents', modified, [
            runtimeHome(modified, runtime),
            file('Info.plist', modified, 1_388),
            dir('MacOS', modified, [file('libjli.dylib', modified, 95_744)]),
            dir('_CodeSignature', modified, [file('CodeResources', modified, 412_166)]),
        ]),
    ])
}

/** A Mojang runtime as the launcher lays it out: `<root>/jre.bundle/Contents/Home`. */
function mojangRuntimeRoot(name: string, modified: string, runtime: RuntimeInfo): FsNode {
    return dir(name, modified, [
        dir('jre.bundle', modified, [
            dir('Contents', modified, [
                runtimeHome(modified, runtime, true),
                file('Info.plist', modified, 1_040),
                dir('MacOS', modified, [file('libjli.dylib', modified, 95_744)]),
            ]),
        ]),
    ])
}

/* ---- Java runtimes of this Mac ------------------------------------------------ */

const TEMURIN_21: RuntimeInfo = {
    major: 21,
    release: { arch: 'x86_64', implementor: 'Eclipse Adoptium' },
    jvmArch: 'x86_64',
}
const ZULU_17: RuntimeInfo = {
    major: 17,
    release: { arch: 'x86_64', implementor: 'Azul Systems, Inc.' },
    jvmArch: 'x86_64',
}
/**
 * `/usr/bin/java` is macOS' launcher stub: it runs the newest JDK under
 * `/Library/Java/JavaVirtualMachines` and has no release file of its own.
 */
const USR_BIN_JAVA: RuntimeInfo = { major: 21, release: null, jvmArch: 'x86_64' }

export const TEMURIN_21_PATH =
    '/Library/Java/JavaVirtualMachines/temurin-21.jdk/Contents/Home/bin/java'
export const ZULU_17_PATH = '/Library/Java/JavaVirtualMachines/zulu-17.jdk/Contents/Home/bin/java'
export const USR_BIN_JAVA_PATH = '/usr/bin/java'

/* ---- The app's own download catalogue -----------------------------------------
   `list_java_feed_vendors` / `list_java_feed_versions` read the JetBrains JDK feed,
   which is not part of the preview's snapshot data. The preview lists the app's own
   catalogue instead: the vendors `DownloadJavaModal` carries branding for, each with the
   Java majors the settings page manages (`supportedJavaVersions`). Archive sizes are
   representative values of a JDK download. */

export interface FeedVendor {
    vendor: string
    product: string
    /** `IMPLEMENTOR` in the release file of the vendor's builds. */
    implementor: string
}

export const FEED_VENDORS: FeedVendor[] = [
    { vendor: 'Alibaba', product: 'Dragonwell', implementor: 'Alibaba' },
    { vendor: 'Amazon', product: 'Corretto', implementor: 'Amazon.com Inc.' },
    { vendor: 'Azul', product: 'Zulu', implementor: 'Azul Systems, Inc.' },
    { vendor: 'BellSoft', product: 'Liberica JDK', implementor: 'BellSoft' },
    { vendor: 'Eclipse', product: 'Temurin', implementor: 'Eclipse Adoptium' },
    { vendor: 'GraalVM', product: 'Community Edition', implementor: 'GraalVM Community' },
    { vendor: 'IBM', product: 'Semeru', implementor: 'IBM Corporation' },
    { vendor: 'JetBrains', product: 'Runtime', implementor: 'JetBrains s.r.o.' },
    { vendor: 'Microsoft', product: 'OpenJDK', implementor: 'Microsoft' },
    { vendor: 'Oracle', product: 'OpenJDK / GraalVM', implementor: 'Oracle Corporation' },
    { vendor: 'SAP', product: 'SapMachine', implementor: 'SAP SE' },
]

export const FEED_MAJOR_VERSIONS = [25, 21, 17, 8]

/** Representative archive size of a JDK build, per major version. */
export const FEED_ARCHIVE_BYTES: Record<number, number> = {
    25: 207_618_048,
    21: 199_229_440,
    17: 188_743_680,
    8: 108_003_328,
}

/** `download_java_from_feed`'s install folder: `<vendor lowercased, spaces as ->-jdk-<major>`. */
export function feedInstallFolder(vendor: string, major: number) {
    return `${vendor.toLowerCase().replace(/ /g, '-')}-jdk-${major}`
}

export function feedRuntimeInfo(vendor: string, major: number): RuntimeInfo {
    const entry = FEED_VENDORS.find((candidate) => candidate.vendor === vendor)
    return {
        major,
        release: { arch: PREVIEW_ARCH, implementor: entry?.implementor ?? vendor },
        jvmArch: PREVIEW_ARCH,
    }
}

/** A runtime the launcher installed into `meta/java_versions`. */
export interface ManagedRuntime {
    folder: string
    modified: string
    layout: 'jdk' | 'mojang'
    runtime: RuntimeInfo
}

export function managedExecutable(entry: ManagedRuntime) {
    return entry.layout === 'mojang'
        ? `${JAVA_VERSIONS_DIR}/${entry.folder}/jre.bundle/Contents/Home/bin/java`
        : `${JAVA_VERSIONS_DIR}/${entry.folder}/Contents/Home/bin/java`
}

function managedRuntimeNode(entry: ManagedRuntime): FsNode {
    if (entry.layout === 'mojang')
        return mojangRuntimeRoot(entry.folder, entry.modified, entry.runtime)
    // A feed archive's top folder renamed to the install folder (`<folder>/Contents/Home`).
    const bundle = jdkBundle(entry.folder, entry.modified, entry.runtime)
    return bundle
}

/** Temurin 25, downloaded earlier through Settings → Download Java. */
export const INITIAL_MANAGED_RUNTIMES: ManagedRuntime[] = [
    {
        folder: feedInstallFolder('Eclipse', 25),
        modified: '2026-06-18T19:12:00Z',
        layout: 'jdk',
        runtime: feedRuntimeInfo('Eclipse', 25),
    },
]

/* ---- Mojang runtimes (`auto_install_java`) ---------------------------------- */

/** `mojang_runtime_component`. */
export function mojangRuntimeComponent(major: number): string | null {
    switch (major) {
        case 8:
            return 'jre-legacy'
        case 16:
            return 'java-runtime-alpha'
        case 17:
            return 'java-runtime-gamma'
        case 21:
            return 'java-runtime-delta'
        case 25:
            return 'java-runtime-epsilon'
        default:
            return null
    }
}

/**
 * The install folder carries the first 12 hex digits of the runtime manifest's SHA-1.
 * Mojang's runtime index is not in the snapshot data, so the preview derives a stable
 * stand-in digest from the component name.
 */
export function mojangInstallFolder(component: string) {
    let hash = 0x811c9dc5
    let digest = ''
    for (let round = 0; round < 2; round++) {
        for (const char of `${component}/${MOJANG_RUNTIME_PLATFORM}/${round}`) {
            hash ^= char.charCodeAt(0)
            hash = Math.imul(hash, 0x01000193) >>> 0
        }
        digest += hash.toString(16).padStart(8, '0')
    }
    return `mojang-${component}-${MOJANG_RUNTIME_PLATFORM}-${digest.slice(0, 12)}`
}

/** Mojang's legacy runtime is a Java 8 JRE whose release file names no implementor. */
export function mojangRuntimeInfo(major: number): RuntimeInfo {
    return {
        major,
        release: { arch: PREVIEW_ARCH, implementor: major === 8 ? null : 'Microsoft' },
        jvmArch: PREVIEW_ARCH,
    }
}

/* ---- The disk ----------------------------------------------------------------- */

export interface MacFileSystemContext {
    managed: ManagedRuntime[]
    /** Instance folders under the launcher's `profiles` directory. */
    instanceFolders: { name: string; modified: string }[]
}

/**
 * Builds the disk for the current state of the launcher's managed runtimes and
 * instances. Mirrors the home folder the import flow of the preview shows.
 */
export function buildMacFileSystem(context: MacFileSystemContext): FsNode {
    const appSupportModified = '2026-10-20T21:03:00Z'
    const appDir = dir('red.ghs.axolotl', appSupportModified, [
        dir('caches', appSupportModified),
        dir('meta', appSupportModified, [
            dir(
                'java_versions',
                context.managed.reduce(
                    (latest, entry) => (entry.modified > latest ? entry.modified : latest),
                    '2026-06-18T19:12:00Z',
                ),
                context.managed.map(managedRuntimeNode),
            ),
            dir('versions', appSupportModified),
        ]),
        dir(
            'profiles',
            appSupportModified,
            context.instanceFolders.map((entry) => dir(entry.name, entry.modified)),
        ),
        file('app.db', appSupportModified, 3_211_264),
    ])

    const home = dir('steve', '2026-10-20T21:03:00Z', [
        dir('Applications', '2026-05-20T10:00:00Z'),
        dir('Desktop', '2026-10-18T20:44:00Z', [
            file('Screenshot 2026-10-18 at 20.44.12.png', '2026-10-18T20:44:00Z', 2_514_944),
        ]),
        dir('Documents', '2026-09-03T14:21:00Z'),
        dir('Downloads', '2026-10-22T18:40:00Z', [
            file('Axolotl.Launcher_1.9.7_universal.dmg', '2026-10-02T18:24:00Z', 137_897_714),
            file('Fabulously.Optimized-v14.1.1.mrpack', '2026-10-22T18:40:00Z', 192_991),
        ]),
        dir(
            'Library',
            '2026-10-20T21:03:00Z',
            [
                dir('Application Support', appSupportModified, [
                    dir('CrashReporter', '2026-10-07T08:20:00Z'),
                    dir('minecraft', '2026-10-20T21:03:00Z'),
                    dir('PrismLauncher', '2026-10-19T09:15:00Z'),
                    appDir,
                ]),
                dir('Caches', '2026-10-22T18:40:00Z'),
                dir('Preferences', '2026-10-22T18:40:00Z'),
            ],
            { hidden: true },
        ),
        dir('Movies', '2026-05-20T10:00:00Z'),
        dir('Music', '2026-05-20T10:00:00Z'),
        dir('Pictures', '2026-08-01T10:00:00Z'),
        dir('Public', '2026-05-20T10:00:00Z'),
    ])

    const javaVirtualMachines = dir('JavaVirtualMachines', '2026-03-12T16:48:00Z', [
        jdkBundle('temurin-21.jdk', '2026-03-12T16:48:00Z', TEMURIN_21),
        jdkBundle('zulu-17.jdk', '2025-11-04T09:31:00Z', ZULU_17),
    ])

    const usrBinModified = '2026-08-06T07:12:00Z'
    const usrBin = dir(
        'bin',
        usrBinModified,
        [
            ['awk', 238_544],
            ['curl', 404_832],
            ['git', 167_120],
            ['jar', 167_216],
            ['jarsigner', 167_216],
            ['java', 167_216],
            ['javac', 167_216],
            ['javadoc', 167_216],
            ['javap', 167_216],
            ['jshell', 167_216],
            ['keytool', 167_216],
            ['python3', 167_120],
            ['ssh', 1_542_208],
            ['zip', 186_240],
        ].map(([name, size]) =>
            file(
                name as string,
                usrBinModified,
                size as number,
                name === 'java' ? { runtime: USR_BIN_JAVA } : {},
            ),
        ),
    )

    const systemModified = '2026-08-06T07:12:00Z'
    return dir('', '2026-05-20T10:00:00Z', [
        dir('Applications', '2026-10-16T11:05:00Z', [
            dir('Axolotl Launcher.app', '2026-10-16T11:05:00Z', [], { package: true }),
            dir('Minecraft.app', '2026-04-02T18:20:00Z', [], { package: true }),
            dir('Safari.app', systemModified, [], { package: true }),
        ]),
        dir('Library', '2026-03-12T16:48:00Z', [
            dir('Application Support', '2026-03-12T16:48:00Z'),
            dir('Fonts', '2026-05-20T10:00:00Z'),
            dir('Java', '2026-03-12T16:48:00Z', [
                dir('Extensions', '2025-11-04T09:31:00Z'),
                javaVirtualMachines,
            ]),
            dir('Preferences', '2026-10-22T18:40:00Z'),
        ]),
        dir('System', systemModified, [dir('Library', systemModified)]),
        dir('Users', '2026-05-20T10:00:00Z', [dir('Shared', '2026-05-20T10:00:00Z'), home]),
        dir('bin', systemModified, [], { hidden: true }),
        dir('opt', '2026-05-20T10:00:00Z', [], { hidden: true }),
        dir('private', systemModified, [], { hidden: true }),
        dir('sbin', systemModified, [], { hidden: true }),
        dir(
            'usr',
            systemModified,
            [
                usrBin,
                dir('lib', systemModified),
                dir('libexec', systemModified, [file('java_home', systemModified, 135_680)]),
                dir('local', '2026-05-20T10:00:00Z'),
                dir('sbin', systemModified),
                dir('share', systemModified),
            ],
            { hidden: true },
        ),
    ])
}

/* ---- Paths ------------------------------------------------------------------ */

/** Resolves `.`/`..`, repeated and trailing slashes; relative paths resolve against `/`. */
export function normalizePath(input: string): string {
    const parts: string[] = []
    for (const part of input.split('/')) {
        if (!part || part === '.') continue
        if (part === '..') parts.pop()
        else parts.push(part)
    }
    return `/${parts.join('/')}`
}

export function joinPath(base: string, name: string) {
    return base === '/' ? `/${name}` : `${base}/${name}`
}

export function parentPath(path: string) {
    const parts = path.split('/').filter(Boolean)
    parts.pop()
    return `/${parts.join('/')}`
}

export function baseName(path: string) {
    return path.split('/').filter(Boolean).pop() ?? ''
}

/** Finds a node; APFS is case-insensitive, so the lookup is too. Returns the on-disk path. */
export function lookupPath(root: FsNode, path: string): { node: FsNode; path: string } | null {
    let node = root
    let resolved = ''
    for (const part of normalizePath(path).split('/').filter(Boolean)) {
        const next = node.children?.find((child) => child.name.toLowerCase() === part.toLowerCase())
        if (!next) return null
        node = next
        resolved += `/${next.name}`
    }
    return { node, path: resolved || '/' }
}
