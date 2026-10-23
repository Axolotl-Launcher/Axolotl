import XIcon from '@modrinth/assets/icons/x.svg?component'
import { useVIntl } from '@modrinth/ui/src/composables/i18n.ts'
import type {
    PopupNotificationButton,
    PopupNotificationProgressItem,
} from '@modrinth/ui/src/providers/popup-notifications.ts'
import { effectScope, markRaw, reactive, watch } from 'vue'

import { type PreviewState, usePreviewDomain } from '../../../../lib/state/index'
import { messages } from '../../messages'
import {
    buildMacFileSystem,
    FEED_ARCHIVE_BYTES,
    FEED_MAJOR_VERSIONS,
    FEED_VENDORS,
    feedInstallFolder,
    feedRuntimeInfo,
    type FsNode,
    HOME,
    INITIAL_MANAGED_RUNTIMES,
    JAVA_BIN,
    JAVA_VERSIONS_DIR,
    joinPath,
    lookupPath,
    managedExecutable,
    type ManagedRuntime,
    mojangInstallFolder,
    mojangRuntimeComponent,
    mojangRuntimeInfo,
    PREVIEW_TOTAL_MEMORY_BYTES,
    TEMURIN_21_PATH,
    USR_BIN_JAVA_PATH,
    ZULU_17_PATH,
} from './mac'

/**
 * The launcher's Java backend (`plugin:jre`, packages/app-lib/src/api/jre.rs and
 * util/jre.rs) and its Java install jobs (install/runner.rs `download_java`), simulated
 * against the preview Mac (`./mac.ts`). The `java_versions` / `java_default_versions`
 * tables, the discovery cache and the launcher's managed runtimes live as long as the
 * window, so what the visitor sets, scans, downloads or removes stays. Install jobs and
 * the `auto_install_java` loading bar surface in the app's download popup exactly as
 * AppActionBar builds it from install jobs and loading bars.
 */

/** `JavaVersion` (state/java_globals.rs), as the frontend receives it. */
export interface JavaVersion {
    parsed_version: number
    version: string
    architecture: string
    path: string
    distribution?: string | null
}

export interface JdkVersionInfo {
    major_version: number
    full_version: string
    vendor: string
    product: string
}

export interface MemoryStatus {
    total_bytes: number
    available_bytes: number
    allocated_mb: number
    optimization_supported: boolean
}

type JobStatus = 'queued' | 'running' | 'canceling' | 'succeeded' | 'failed' | 'canceled'
type JavaStep = 'fetching_metadata' | 'downloading' | 'extracting' | 'validating'

/** The parts of an `InstallJobSnapshot` a `download_java` job fills. */
export interface JavaInstallJob {
    job_id: string
    vendor: string
    version: number
    status: JobStatus
    phase: 'preparing_instance' | 'preparing_java' | 'rolling_back' | 'completed'
    step: JavaStep | null
    progress: { current: number; total: number } | null
    error: { code: string; message: string } | null
    created: string
}

/** A `LoadingBarType::JavaDownload` bar of `auto_install_java`. */
interface JavaLoadingBar {
    id: string
    version: number
    current: number
    total: number
    message: string
}

interface JavaDomain {
    /** `java_versions` rows, in insertion order. */
    versions: JavaVersion[]
    /** `java_default_versions`: major version -> path. */
    defaults: Record<number, string>
    /** `discovered_javas`: executable paths of the last system scan. */
    discovered: string[]
    lastScanAt: number | null
    managed: ManagedRuntime[]
    feedCached: boolean
    jobs: JavaInstallJob[]
    loadingBars: JavaLoadingBar[]
    /** Bumped by `emit_java_discovery_update`. */
    discoveryEvents: number
    memory: { available: number; sampledAt: number }
    /** Bumped whenever the disk changes (installs, instance folders). */
    diskVersion: number
    /** The open panel remembers the app's last folder and the hidden-files toggle. */
    openPanel: { directory: string; showHidden: boolean }
}

/** A serialized backend error (`TheseusSerializableError::Theseus`). */
export interface TheseusError {
    field_name: 'Theseus'
    message: string
}

function backendError(message: string): TheseusError {
    return { field_name: 'Theseus', message }
}

const initialVersions = (): JavaVersion[] => [
    {
        parsed_version: 25,
        version: '25',
        architecture: 'amd64',
        path: managedExecutable(INITIAL_MANAGED_RUNTIMES[0]),
        distribution: 'Eclipse Adoptium',
    },
    {
        parsed_version: 21,
        version: '21',
        architecture: 'amd64',
        path: TEMURIN_21_PATH,
        distribution: 'Eclipse Adoptium',
    },
    {
        parsed_version: 21,
        version: '21',
        architecture: 'x86_64',
        path: USR_BIN_JAVA_PATH,
        distribution: null,
    },
    {
        parsed_version: 17,
        version: '17',
        architecture: 'amd64',
        path: ZULU_17_PATH,
        distribution: 'Azul Systems, Inc.',
    },
]

/** Durations of the backend's work on this Mac, in milliseconds. */
const TIMING = {
    invoke: 40,
    readReleaseFile: 30,
    runJvm: 420,
    quickScan: 900,
    keywordScan: 2600,
    exhaustiveScan: 7800,
    feedDownload: 1100,
    feedCached: 120,
    jobFetchMetadata: 900,
    jobFetchMetadataCached: 260,
    jobDownload: 6200,
    jobExtract: 1300,
    jobValidate: 420,
    mojangIndex: 650,
    mojangManifest: 380,
    mojangDownload: 5200,
    mojangInstall: 900,
}

const JAVA_RESCAN_DEBOUNCE_MS = 60_000

function randomId() {
    const hex = () =>
        Math.floor(Math.random() * 0x10000)
            .toString(16)
            .padStart(4, '0')
    return `${hex()}${hex()}-${hex()}-4${hex().slice(1)}-a${hex().slice(1)}-${hex()}${hex()}${hex()}`
}

/** `normalize_release_arch`: release files say `x86_64`, the JVM's `os.arch` form is `amd64`. */
function normalizeReleaseArch(arch: string) {
    const lower = arch.trim().toLowerCase()
    if (lower === 'x86_64' || lower === 'amd64') return 'amd64'
    if (lower === 'aarch64' || lower === 'arm64') return 'aarch64'
    return lower
}

/** Round half to even, as Rust's `round_ties_even`. */
function roundTiesEven(value: number) {
    const floor = Math.floor(value)
    const diff = value - floor
    if (Math.abs(diff - 0.5) < 1e-9) return floor % 2 === 0 ? floor : floor + 1
    return Math.round(value)
}

/** `automatic_memory_max_mb`: the launch heap from available memory in four stages. */
export function automaticMemoryMaxMb(availableBytes: number, modCount: number, modded: boolean) {
    let availableGib = roundTiesEven((availableBytes / 1024 ** 3) * 10) / 10
    const [minimum, target1, target2, target3] = modded
        ? [0.5 + modCount / 150, 1.5 + modCount / 90, 2.7 + modCount / 50, 4.5 + modCount / 25]
        : [0.5, 1.5, 2.5, 4.0]
    let allocated = 0
    const stages: [number, number][] = [
        [target1, 1.0],
        [target2 - target1, 0.7],
        [target3 - target2, 0.4],
        [target3, 0.15],
    ]
    for (const [delta, ratio] of stages) {
        allocated += Math.min(availableGib * ratio, delta)
        availableGib -= delta / ratio
        if (availableGib < 0.1) break
    }
    const allocatedGib = roundTiesEven(Math.max(allocated, minimum) * 10) / 10
    return Math.max(Math.floor(allocatedGib * 1024), 512)
}

function createJavaDomain(state: PreviewState) {
    const { formatMessage, locale } = useVIntl()
    const data = reactive<JavaDomain>({
        versions: initialVersions(),
        defaults: {
            25: managedExecutable(INITIAL_MANAGED_RUNTIMES[0]),
            21: TEMURIN_21_PATH,
            17: ZULU_17_PATH,
        },
        discovered: [
            managedExecutable(INITIAL_MANAGED_RUNTIMES[0]),
            TEMURIN_21_PATH,
            USR_BIN_JAVA_PATH,
            ZULU_17_PATH,
        ],
        lastScanAt: null,
        managed: INITIAL_MANAGED_RUNTIMES.map((entry) => ({ ...entry })),
        feedCached: false,
        jobs: [],
        loadingBars: [],
        discoveryEvents: 0,
        memory: { available: 7.32 * 1024 ** 3, sampledAt: 0 },
        diskVersion: 0,
        openPanel: { directory: HOME, showHidden: false },
    })

    const cancelRequests = new Set<string>()
    const jobWaiters = new Map<string, ((job: JavaInstallJob) => void)[]>()
    let installLock: Promise<unknown> = Promise.resolve()
    let scanLock: Promise<unknown> = Promise.resolve()
    /** Watchers of the domain: they live with the window, not with the page that opened it. */
    const scope = effectScope(true)

    /** Work on the window's clock; the clock drops what is pending when the window closes. */
    function delay(ms: number) {
        return new Promise<void>((resolve) => {
            state.clock.after(ms, resolve)
        })
    }

    /** Serializes work behind `JAVA_INSTALL_LOCK` / `JAVA_SCAN_STATE`. */
    function locked<T>(kind: 'install' | 'scan', task: () => Promise<T>): Promise<T> {
        const previous = kind === 'install' ? installLock : scanLock
        const run = previous.catch(() => {}).then(task)
        if (kind === 'install') installLock = run
        else scanLock = run
        return run
    }

    /* ---- The disk -------------------------------------------------------- */

    let fs: FsNode | null = null
    function disk(): FsNode {
        fs ??= markRaw(
            buildMacFileSystem({
                managed: data.managed,
                instanceFolders: state.instances.instances.value.map((instance) => ({
                    name: instance.path.split('/').pop() ?? instance.name,
                    modified: instance.modified,
                })),
            }),
        )
        return fs
    }
    function invalidateDisk() {
        fs = null
        data.diskVersion += 1
    }
    scope.run(() => {
        watch(
            () => state.instances.instances.value.map((instance) => instance.path),
            invalidateDisk,
        )
    })

    /* ---- util/jre.rs ----------------------------------------------------- */

    /** `check_java_at_filepath`. */
    async function checkJavaAtFilepath(path: string): Promise<JavaVersion> {
        const found = lookupPath(disk(), path)
        if (!found) {
            await delay(TIMING.readReleaseFile)
            throw backendError(`I/O error: No such file or directory (os error 2), path: ${path}`)
        }
        const javaPath = found.node.name === JAVA_BIN ? found.path : joinPath(found.path, JAVA_BIN)
        const java = found.node.name === JAVA_BIN ? found : lookupPath(disk(), javaPath)
        if (!java) {
            await delay(TIMING.readReleaseFile)
            throw backendError(`JRE error: No executable found at ${javaPath}`)
        }
        const runtime = java.node.runtime
        if (runtime?.release) {
            await delay(TIMING.readReleaseFile)
            return {
                parsed_version: runtime.major,
                path: java.path,
                version: String(runtime.major),
                architecture: normalizeReleaseArch(runtime.release.arch),
                distribution: runtime.release.implementor,
            }
        }
        await delay(TIMING.runJvm)
        if (java.node.kind === 'dir') {
            throw backendError('I/O (std) error: Permission denied (os error 13)')
        }
        if (!runtime)
            throw backendError(`JRE error: Could not check Java version at path ${java.path}`)
        return {
            parsed_version: runtime.major,
            path: java.path,
            version: String(runtime.major),
            architecture: runtime.jvmArch,
            distribution: null,
        }
    }

    /** `collect_candidate_paths`: what a quick scan checks on macOS (the BFS adds nothing here). */
    function candidatePaths(): string[] {
        const root = disk()
        const paths = new Set<string>()
        // PATH of a GUI app on macOS.
        for (const entry of ['/usr/bin', '/bin', '/usr/sbin', '/sbin']) paths.add(entry)
        // `get_all_autoinstalled_jre_path`
        for (const entry of lookupPath(root, JAVA_VERSIONS_DIR)?.node.children ?? []) {
            if (entry.name.startsWith('.')) continue
            const base = joinPath(JAVA_VERSIONS_DIR, entry.name)
            paths.add(`${base}/Contents/Home/bin/java`)
            paths.add(`${base}/jre.bundle/Contents/Home/bin/java`)
        }
        // `get_common_install_paths` (macOS)
        for (const vm of lookupPath(root, '/Library/Java/JavaVirtualMachines')?.node.children ??
            []) {
            paths.add(`/Library/Java/JavaVirtualMachines/${vm.name}/Contents/Home/bin`)
        }
        return [...paths]
    }

    function upsertVersion(java: JavaVersion) {
        const index = data.versions.findIndex((row) => row.path === java.path)
        if (index > -1) data.versions[index] = { ...java }
        else data.versions.push({ ...java })
    }

    /** `refresh_discovered_javas`. */
    async function refreshDiscovered(mode: 'quick' | 'keyword' | 'exhaustive') {
        await delay(
            mode === 'quick'
                ? TIMING.quickScan
                : mode === 'keyword'
                  ? TIMING.keywordScan
                  : TIMING.exhaustiveScan,
        )
        const found = new Map<string, JavaVersion>()
        for (const candidate of candidatePaths()) {
            const java = await checkJavaAtFilepath(candidate).catch(() => null)
            if (java) found.set(java.path, java)
        }
        const previous = new Set(data.discovered)
        data.discovered = [...found.keys()]
        // Also upserted into java_versions so they appear in the settings list.
        for (const java of found.values()) upsertVersion(java)
        const changed =
            previous.size !== found.size || [...found.keys()].some((path) => !previous.has(path))
        if (changed) data.discoveryEvents += 1
        data.lastScanAt = Date.now()
        return [...found.values()]
    }

    /** `validate_cached_javas`: drops cache entries whose executable is gone. */
    function validateCached(): string[] {
        data.discovered = data.discovered.filter((path) => !!lookupPath(disk(), path))
        return data.discovered
    }

    /** `schedule_background_java_rescan`, debounced to once a minute. */
    function scheduleBackgroundRescan() {
        void locked('scan', async () => {
            if (
                data.lastScanAt !== null &&
                Date.now() - data.lastScanAt < JAVA_RESCAN_DEBOUNCE_MS
            ) {
                return
            }
            await refreshDiscovered('keyword')
        })
    }

    /** `register_managed_java_versions`. */
    async function registerManaged() {
        for (const entry of data.managed) {
            const java = await checkJavaAtFilepath(managedExecutable(entry)).catch(() => null)
            if (!java) continue
            const folderVersion = Number(entry.folder.split('-jdk-').pop())
            const setDefault =
                entry.folder.includes('-jdk-') &&
                folderVersion === java.parsed_version &&
                !defaultFor(java.parsed_version)
            upsertVersion(java)
            if (setDefault) data.defaults[java.parsed_version] = java.path
            if (!data.discovered.includes(java.path)) data.discovered.push(java.path)
        }
    }

    function defaultFor(major: number): JavaVersion | null {
        const path = data.defaults[major]
        if (!path) return null
        return (
            data.versions.find((row) => row.path === path && row.parsed_version === major) ?? null
        )
    }

    /* ---- api/jre.rs -------------------------------------------------------- */

    async function getJavaVersions(): Promise<JavaVersion[]> {
        await delay(TIMING.invoke)
        await registerManaged()
        return data.versions.map((row) => ({ ...row }))
    }

    async function getJavaDefaultVersions(): Promise<JavaVersion[]> {
        await delay(TIMING.invoke)
        await registerManaged()
        return Object.keys(data.defaults)
            .map(Number)
            .map((major) => defaultFor(major))
            .filter((row): row is JavaVersion => !!row)
            .sort((a, b) => b.parsed_version - a.parsed_version)
            .map((row) => ({ ...row }))
    }

    async function setJavaVersion(java: JavaVersion) {
        await delay(TIMING.invoke)
        upsertVersion(java)
    }

    async function setJavaDefaultVersion(major: number, path: string): Promise<JavaVersion> {
        const java = await checkJavaAtFilepath(path)
        if (java.parsed_version !== major) {
            throw backendError(
                `Invalid input: Java ${java.parsed_version} cannot be used as the Java ${major} default`,
            )
        }
        upsertVersion(java)
        data.defaults[major] = java.path
        return { ...java }
    }

    async function removeJavaDefaultVersion(major: number) {
        await delay(TIMING.invoke)
        Reflect.deleteProperty(data.defaults, major)
    }

    async function removeJavaVersion(path: string) {
        await delay(TIMING.invoke)
        for (const [major, defaultPath] of Object.entries(data.defaults)) {
            if (defaultPath === path) Reflect.deleteProperty(data.defaults, Number(major))
        }
        data.versions = data.versions.filter((row) => row.path !== path)
    }

    /** `get_available_jres`. */
    async function getAvailableJres(fullScan: boolean, forceFresh: boolean, exhaustive: boolean) {
        if (fullScan || exhaustive) {
            return locked('scan', () => refreshDiscovered(exhaustive ? 'exhaustive' : 'keyword'))
        }
        if (!forceFresh && validateCached().length) {
            scheduleBackgroundRescan()
            return
        }
        return locked('scan', async () => {
            if (!forceFresh && validateCached().length) return
            await refreshDiscovered('quick')
        })
    }

    async function findFilteredJres(
        version: number | null,
        fullScan = false,
        forceFresh = false,
        exhaustive = false,
    ): Promise<JavaVersion[]> {
        await delay(TIMING.invoke)
        await getAvailableJres(fullScan, forceFresh, exhaustive)
        const rows = data.versions.map((row) => ({ ...row }))
        return version == null ? rows : rows.filter((row) => row.parsed_version === version)
    }

    async function testJre(path: string, major: number): Promise<boolean> {
        const java = await checkJavaAtFilepath(path).catch(() => null)
        return !!java && java.parsed_version === major
    }

    function getMaxMemory(): number {
        return PREVIEW_TOTAL_MEMORY_BYTES / 1024
    }

    /** Free + inactive + purgeable pages, minus what running games hold. */
    function sampleAvailableBytes() {
        const now = Date.now()
        if (now - data.memory.sampledAt >= 900) {
            const target = 7.32 * 1024 ** 3
            const drift = (Math.random() - 0.5) * 72 * 1024 ** 2
            const pull = (target - data.memory.available) * 0.18
            data.memory.available = Math.min(
                7.9 * 1024 ** 3,
                Math.max(6.7 * 1024 ** 3, data.memory.available + drift + pull),
            )
            data.memory.sampledAt = now
        }
        const games = state.processes.processes.value.filter((entry) => entry.stage === 'running')
        const held = games.reduce((sum, entry) => {
            const memory = entry.instance.memory
            const heapMb = memory && !memory.automatic ? memory.maximum : 4096
            return sum + (heapMb + 640) * 1024 ** 2
        }, 0)
        return Math.max(0.6 * 1024 ** 3, data.memory.available - held)
    }

    /** `get_memory_status`; without an instance the automatic heap assumes no mods. */
    async function getMemoryStatus(
        _instanceId: string | null,
        requestedMb: number,
        automatic: boolean,
    ): Promise<MemoryStatus> {
        await delay(12)
        const available = sampleAvailableBytes()
        return {
            total_bytes: PREVIEW_TOTAL_MEMORY_BYTES,
            available_bytes: available,
            allocated_mb: automatic ? automaticMemoryMaxMb(available, 0, false) : requestedMb,
            // `optimization_supported()` is Windows-only.
            optimization_supported: false,
        }
    }

    async function optimizeMemory() {
        await delay(TIMING.invoke)
        return { supported: false, reclaimed_bytes: 0 }
    }

    /* ---- The JetBrains JDK feed -------------------------------------------- */

    async function fetchFeed() {
        await delay(data.feedCached ? TIMING.feedCached : TIMING.feedDownload)
        data.feedCached = true
    }

    async function listJavaFeedVendors(): Promise<string[]> {
        await fetchFeed()
        return FEED_VENDORS.map((entry) => entry.vendor).sort()
    }

    async function listJavaFeedVersions(vendor: string): Promise<JdkVersionInfo[]> {
        await fetchFeed()
        const entry = FEED_VENDORS.find((candidate) => candidate.vendor === vendor)
        if (!entry) return []
        return FEED_MAJOR_VERSIONS.map((major) => ({
            major_version: major,
            full_version: String(major),
            vendor: entry.vendor,
            product: entry.product,
        }))
    }

    /* ---- Install jobs ------------------------------------------------------ */

    function snapshot(job: JavaInstallJob): JavaInstallJob {
        return {
            ...job,
            progress: job.progress ? { ...job.progress } : null,
            error: job.error ? { ...job.error } : null,
        }
    }

    function jobById(jobId: string) {
        return data.jobs.find((job) => job.job_id === jobId)
    }

    const isFinished = (job: JavaInstallJob) =>
        job.status === 'succeeded' || job.status === 'failed' || job.status === 'canceled'

    function settle(job: JavaInstallJob) {
        const waiters = jobWaiters.get(job.job_id) ?? []
        jobWaiters.delete(job.job_id)
        for (const resolve of waiters) resolve(snapshot(job))
    }

    function finishCanceled(job: JavaInstallJob) {
        job.status = 'canceled'
        job.phase = 'rolling_back'
        job.step = null
        job.progress = null
        settle(job)
    }

    /** Removes an install folder again (the job's rollback of a canceled download). */
    function rollbackManaged(folder: string) {
        const executable = `${JAVA_VERSIONS_DIR}/${folder}/Contents/Home/bin/java`
        data.managed = data.managed.filter((entry) => entry.folder !== folder)
        data.versions = data.versions.filter((row) => row.path !== executable)
        data.discovered = data.discovered.filter((path) => path !== executable)
        for (const [major, path] of Object.entries(data.defaults)) {
            if (path === executable) Reflect.deleteProperty(data.defaults, Number(major))
        }
        invalidateDisk()
    }

    /** install/runner `DownloadJava` -> `download_java_from_feed_with_reporter`. */
    async function runDownloadJob(jobId: string) {
        const live = () => jobById(jobId)
        const canceled = () => cancelRequests.has(jobId)
        let installedFolder: string | null = null
        await locked('install', async () => {
            const job = live()
            if (!job || job.status !== 'queued') return
            job.status = 'running'
            job.phase = 'preparing_java'
            job.step = 'fetching_metadata'
            job.progress = { current: 1, total: 4 }
            await delay(data.feedCached ? TIMING.jobFetchMetadataCached : TIMING.jobFetchMetadata)
            data.feedCached = true
            if (canceled()) return

            const total = FEED_ARCHIVE_BYTES[job.version] ?? FEED_ARCHIVE_BYTES[21]
            job.step = 'downloading'
            job.progress = { current: 0, total }
            await new Promise<void>((resolve) => {
                let elapsed = 0
                const stop = state.clock.every((delta) => {
                    const current = live()
                    if (!current || canceled()) {
                        stop()
                        resolve()
                        return
                    }
                    elapsed += delta
                    // The transfer rate wanders a little, as a real download's does.
                    const share = Math.min(1, elapsed / TIMING.jobDownload)
                    const wobble = share < 1 ? Math.sin(elapsed / 410) * 0.012 : 0
                    const bytes = Math.round(total * Math.min(1, Math.max(0, share + wobble)))
                    current.progress = {
                        current: Math.max(current.progress?.current ?? 0, bytes),
                        total,
                    }
                    if (share >= 1) {
                        stop()
                        resolve()
                    }
                })
            })
            if (canceled()) return

            job.step = 'extracting'
            job.progress = { current: 3, total: 4 }
            await delay(TIMING.jobExtract)
            if (canceled()) return
            const managed: ManagedRuntime = {
                folder: feedInstallFolder(job.vendor, job.version),
                modified: new Date().toISOString(),
                layout: 'jdk',
                runtime: feedRuntimeInfo(job.vendor, job.version),
            }
            data.managed = [
                ...data.managed.filter((entry) => entry.folder !== managed.folder),
                managed,
            ]
            installedFolder = managed.folder
            invalidateDisk()

            job.step = 'validating'
            job.progress = { current: 4, total: 4 }
            await delay(TIMING.jobValidate)
            if (canceled()) return
            const java = await checkJavaAtFilepath(managedExecutable(managed))
            upsertVersion(java)
            data.defaults[java.parsed_version] = java.path
            if (!data.discovered.includes(java.path)) data.discovered.push(java.path)

            const finished = live()
            if (!finished) return
            finished.status = 'succeeded'
            finished.phase = 'completed'
            finished.step = null
            finished.progress = null
            settle(finished)
        }).catch((error: unknown) => {
            const job = live()
            if (!job) return
            job.status = 'failed'
            job.error = {
                code: 'java_error',
                message:
                    (error as TheseusError | undefined)?.message ??
                    `Failed to download Java from ${job.vendor}`,
            }
            settle(job)
        })
        const job = live()
        if (job && canceled() && job.status === 'canceling') {
            await delay(600)
            if (installedFolder) rollbackManaged(installedFolder)
            finishCanceled(job)
        }
        cancelRequests.delete(jobId)
    }

    /** `download_java`: queues the install job and returns its snapshot. */
    async function downloadJava(vendor: string, version: number): Promise<JavaInstallJob> {
        await delay(TIMING.invoke)
        const job: JavaInstallJob = {
            job_id: randomId(),
            vendor,
            version,
            status: 'queued',
            phase: 'preparing_instance',
            step: null,
            progress: null,
            error: null,
            created: new Date().toISOString(),
        }
        data.jobs.push(job)
        void runDownloadJob(job.job_id)
        return snapshot(job)
    }

    /** `install_job_cancel`. */
    async function cancelJob(jobId: string) {
        const job = jobById(jobId)
        if (!job || !['queued', 'running'].includes(job.status)) return
        cancelRequests.add(jobId)
        job.error = { code: 'canceled', message: 'Install was canceled' }
        if (job.status === 'queued') {
            finishCanceled(job)
            return
        }
        job.status = 'canceling'
        job.phase = 'rolling_back'
        job.step = null
        job.progress = null
    }

    /** `install_job_dismiss`. */
    async function dismissJob(jobId: string) {
        data.jobs = data.jobs.filter((job) => job.job_id !== jobId)
    }

    /** `wait_for_install_job`. */
    function waitForInstallJob(jobId: string): Promise<JavaInstallJob> {
        return new Promise((resolve, reject) => {
            const settleWith = (job: JavaInstallJob) => {
                if (job.status === 'succeeded') resolve(job)
                else
                    reject(
                        new Error(job.error?.message ?? `Install job ${job.job_id} ${job.status}`),
                    )
            }
            const job = jobById(jobId)
            if (!job) {
                reject(new Error(`Install job ${jobId} was not found`))
                return
            }
            if (isFinished(job)) {
                settleWith(snapshot(job))
                return
            }
            jobWaiters.set(jobId, [...(jobWaiters.get(jobId) ?? []), settleWith])
        })
    }

    /** `auto_install_java`: Mojang's runtime for the major, behind a loading bar. */
    async function autoInstallJava(major: number): Promise<string | null> {
        return locked('install', async () => {
            const bar: JavaLoadingBar = {
                id: randomId(),
                version: major,
                current: 0,
                total: 100,
                message: 'Fetching java version',
            }
            data.loadingBars.push(bar)
            const emit = (delta: number, message?: string) => {
                const current = data.loadingBars.find((entry) => entry.id === bar.id)
                if (!current) return
                current.current = Math.min(current.total, current.current + delta)
                if (message) current.message = message
            }
            try {
                const component = mojangRuntimeComponent(major)
                await delay(TIMING.mojangIndex)
                if (!component) {
                    // No Mojang runtime: the Azul metadata API has no package either.
                    throw backendError(
                        `Error launching Minecraft: No Java Version found for Java version ${major}, OS macos, and Architecture x86_64`,
                    )
                }
                await delay(TIMING.mojangManifest)
                const folder = mojangInstallFolder(component)
                if (!data.managed.some((entry) => entry.folder === folder)) {
                    emit(10, 'Downloading java version')
                    await delay(TIMING.mojangDownload)
                    emit(80, 'Installing java runtime')
                    await delay(TIMING.mojangInstall)
                    data.managed = [
                        ...data.managed,
                        {
                            folder,
                            modified: new Date().toISOString(),
                            layout: 'mojang',
                            runtime: mojangRuntimeInfo(major),
                        },
                    ]
                    invalidateDisk()
                }
                const executable = `${JAVA_VERSIONS_DIR}/${folder}/jre.bundle/Contents/Home/bin/java`
                if (!(await testJre(executable, major))) {
                    throw backendError(
                        `Error launching Minecraft: Downloaded Java runtime did not report major version ${major}`,
                    )
                }
                emit(10, 'Done installing java')
                await delay(300)
                return executable
            } finally {
                data.loadingBars = data.loadingBars.filter((entry) => entry.id !== bar.id)
            }
        })
    }

    /* ---- The download popup (AppActionBar) ------------------------------------------
       One `download` popup lists the Java install jobs (title "Installs") and the
       `auto_install_java` loading bars (title "Downloads" when there are no jobs),
       the way AppActionBar's `buildDownloadItems` / `updateNotification` do. */

    let popupId: string | number | null = null
    let dismissedSignature: string | null = null

    const installTitle = (job: JavaInstallJob) => `Java ${job.version} (${job.vendor})`

    const javaStepMessage = {
        fetching_metadata: messages.appInstallPhasePreparingJavaFetchingMetadata,
        downloading: messages.appInstallPhasePreparingJavaDownloading,
        extracting: messages.appInstallPhasePreparingJavaExtracting,
        validating: messages.appInstallPhasePreparingJavaValidating,
    }

    function jobText(job: JavaInstallJob) {
        if (job.status === 'succeeded') return formatMessage(messages.appDownloadsStatusSucceeded)
        if (job.status === 'canceled') {
            return formatMessage(messages.appActionBarInstallSummaryCanceled)
        }
        if (job.status === 'failed') {
            return formatMessage(messages.appActionBarInstallSummaryJavaSetupFailed)
        }
        if (job.phase === 'preparing_java' && job.step) {
            return formatMessage(javaStepMessage[job.step], { version: job.version })
        }
        if (job.phase === 'rolling_back') return formatMessage(messages.appInstallPhaseRollingBack)
        return formatMessage(messages.appInstallPhasePreparingInstance)
    }

    function jobItem(job: JavaInstallJob): PopupNotificationProgressItem {
        const finished = isFinished(job)
        const progress = job.progress && job.progress.total > 0 ? job.progress : null
        const buttons: PopupNotificationButton[] = []
        if (job.status === 'queued' || job.status === 'running') {
            buttons.push({
                label: formatMessage(messages.appActionBarInstallCancel),
                icon: markRaw(XIcon),
                color: 'red',
                keepOpen: true,
                action: () => cancelJob(job.job_id),
            })
        }
        return {
            id: job.job_id,
            title: installTitle(job),
            text: jobText(job),
            iconUrl: null,
            progress:
                job.status === 'succeeded'
                    ? 1
                    : progress && !finished
                      ? Math.max(0, Math.min(1, progress.current / progress.total))
                      : 0,
            waiting: (job.status === 'queued' || job.status === 'running') && !progress,
            showProgress: !finished,
            wrapText: job.status === 'failed',
            progressType:
                finished || !progress
                    ? undefined
                    : job.step === 'downloading'
                      ? 'bytes'
                      : 'percentage',
            progressCurrent: !finished && progress ? progress.current : undefined,
            progressTotal: !finished && progress ? progress.total : undefined,
            buttons,
            onDismiss: finished ? () => dismissJob(job.job_id) : undefined,
        }
    }

    function barItem(bar: JavaLoadingBar): PopupNotificationProgressItem {
        const progress = Math.max(0, Math.min(1, bar.current / bar.total))
        return {
            id: bar.id,
            title: formatMessage(messages.appActionBarDownloadingJava, { version: bar.version }),
            text: bar.message,
            iconUrl: null,
            progress,
            waiting: false,
            progressType: 'percentage',
            progressCurrent: progress * bar.total,
            progressTotal: bar.total,
        }
    }

    function syncPopup() {
        const rank = (job: JavaInstallJob) =>
            isFinished(job) ? 0 : job.status === 'queued' ? 2 : 1
        const jobs = [...data.jobs].sort(
            (a, b) => rank(a) - rank(b) || a.created.localeCompare(b.created),
        )
        const items = [...jobs.map(jobItem), ...data.loadingBars.map(barItem)]
        const signature = [
            ...data.jobs.map((job) => `${job.job_id}:${job.status}:${job.phase}`),
            ...data.loadingBars.map((bar) => bar.id),
        ]
            .sort()
            .join('|')
        const current = () =>
            popupId === null
                ? undefined
                : state.popups.getNotifications().find((entry) => entry.id === popupId)

        if (!items.length) {
            const existing = current()
            if (existing) state.popups.removeNotification(existing.id)
            popupId = null
            dismissedSignature = null
            return
        }
        if (popupId !== null && !current()) {
            // Closed by the visitor: it stays closed until the jobs move on.
            popupId = null
            dismissedSignature = signature
        }
        if (dismissedSignature !== null) {
            if (dismissedSignature === signature) return
            dismissedSignature = null
        }
        const title = data.jobs.length
            ? formatMessage(messages.appActionBarInstalls)
            : formatMessage(messages.appActionBarDownloads)
        const autoCloseMs = items.every((item) => item.showProgress === false) ? 30_000 : null
        const existing = current()
        if (existing) {
            existing.title = title
            existing.text = undefined
            existing.progressItems = items
            if (existing.autoCloseMs !== autoCloseMs) {
                existing.autoCloseMs = autoCloseMs
                if (!existing.collapsed) state.popups.setNotificationTimer(existing)
            }
            return
        }
        const created = state.popups.addPopupNotification({
            title,
            type: 'download',
            autoCloseMs,
            progressItems: items,
        })
        popupId = created.id
    }

    scope.run(() => {
        watch(
            () => [
                locale.value,
                data.jobs.map((job) => ({ ...job, progress: job.progress && { ...job.progress } })),
                data.loadingBars.map((bar) => ({ ...bar })),
            ],
            syncPopup,
        )
    })

    function dispose() {
        scope.stop()
        if (popupId !== null) state.popups.removeNotification(popupId)
    }

    const api = {
        data,
        disk,
        getJavaVersions,
        getJavaDefaultVersions,
        setJavaVersion,
        setJavaDefaultVersion,
        removeJavaDefaultVersion,
        removeJavaVersion,
        findFilteredJres,
        getJre: checkJavaAtFilepath,
        testJre,
        autoInstallJava,
        getMaxMemory,
        getMemoryStatus,
        optimizeMemory,
        listJavaFeedVendors,
        listJavaFeedVersions,
        downloadJava,
        waitForInstallJob,
        cancelJob,
    }
    return { value: api, dispose }
}

export type JavaBackend = ReturnType<typeof createJavaDomain>['value']

/** The Java backend of this window (`@/helpers/jre` and `wait_for_install_job`). */
export function useJre(): JavaBackend {
    return usePreviewDomain('settings:java', createJavaDomain)
}
