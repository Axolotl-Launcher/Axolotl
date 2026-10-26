import { createHash } from 'node:crypto'
import fs from 'node:fs'
import path from 'node:path'
import { pathToFileURL } from 'node:url'
import { parseArgs } from 'node:util'

import {
    app,
    files,
    requireFile,
    requireProfile,
    root,
    run,
    sha256,
    version,
} from './desktop-build-utils.mjs'

const PRODUCT_NAME = 'Axolotl Launcher'
const PACKAGE_NAME = 'axolotl-launcher'
const HOMEPAGE = 'https://axlmc.org'
const DESCRIPTION = 'Axolotl Launcher is a cross-platform Minecraft launcher'
const RECOMMENDS =
    'openjdk-21-jre-headless | openjdk-17-jre-headless | openjdk-8-jre-headless | default-jre-headless'

export const architectures = {
    aarch64: { deb: 'arm64', elf: 183 },
    loongarch64: { deb: 'loong64', elf: 258 },
    riscv64: { deb: 'riscv64', elf: 243 },
    x86_64: { deb: 'amd64', elf: 62 },
}

export function normalizeArchitecture(architecture) {
    switch (String(architecture).trim().toLowerCase()) {
        case 'amd64':
        case 'x64':
        case 'x86_64':
            return 'x86_64'
        case 'arm64':
        case 'aarch64':
            return 'aarch64'
        case 'loong64':
        case 'loongarch64':
        case 'loongarch64el':
            return 'loongarch64'
        case 'riscv64':
        case 'riscv64gc':
            return 'riscv64'
        default:
            throw new Error(`Unsupported Linux package architecture: ${architecture}`)
    }
}

export function architectureFromTarget(target) {
    if (!target) throw new Error('A target triple or --arch is required')
    const architecture = target.split('-')[0]
    return normalizeArchitecture(architecture)
}

export function debArchitecture(architecture) {
    const normalized = normalizeArchitecture(architecture)
    return architectures[normalized].deb
}

export function elfArchitecture(file) {
    const descriptor = fs.openSync(file, 'r')
    const header = Buffer.alloc(20)
    try {
        if (fs.readSync(descriptor, header, 0, header.length, 0) !== header.length)
            throw new Error(`Unable to read ELF header: ${file}`)
    } finally {
        fs.closeSync(descriptor)
    }
    if (
        header[0] !== 0x7f ||
        header[1] !== 0x45 ||
        header[2] !== 0x4c ||
        header[3] !== 0x46 ||
        header[4] !== 2 ||
        header[5] !== 1
    )
        throw new Error(`Expected a 64-bit little-endian ELF executable: ${file}`)
    const machine = header.readUInt16LE(18)
    for (const [architecture, details] of Object.entries(architectures)) {
        if (details.elf === machine) return architecture
    }
    throw new Error(`Unsupported ELF machine ${machine}: ${file}`)
}

function pngDimensions(file) {
    const descriptor = fs.openSync(file, 'r')
    const header = Buffer.alloc(24)
    try {
        if (fs.readSync(descriptor, header, 0, header.length, 0) !== header.length)
            throw new Error(`Unable to read PNG header: ${file}`)
    } finally {
        fs.closeSync(descriptor)
    }
    if (
        header[0] !== 0x89 ||
        header[1] !== 0x50 ||
        header[2] !== 0x4e ||
        header[3] !== 0x47 ||
        header[12] !== 0x49 ||
        header[13] !== 0x48 ||
        header[14] !== 0x44 ||
        header[15] !== 0x52
    )
        throw new Error(`Expected a PNG icon: ${file}`)
    return {
        width: header.readUInt32BE(16),
        height: header.readUInt32BE(20),
    }
}

function copyTree(source, destination) {
    files(source)
    fs.cpSync(source, destination, { recursive: true, preserveTimestamps: true })
}

function writeFile(file, contents, mode = 0o644) {
    fs.mkdirSync(path.dirname(file), { recursive: true })
    fs.writeFileSync(file, contents)
    fs.chmodSync(file, mode)
}

function md5(file) {
    return createHash('md5').update(fs.readFileSync(file)).digest('hex')
}

function desktopEntry(config) {
    const associations = config.bundle?.fileAssociations ?? []
    const schemes = config.plugins?.['deep-link']?.desktop?.schemes ?? []
    const mimeTypes = [
        ...associations.flatMap((association) =>
            association.mimeType ? [association.mimeType] : [],
        ),
        ...schemes.map((scheme) => `x-scheme-handler/${scheme}`),
    ]
    const exec = PRODUCT_NAME.includes(' ') ? `"${PRODUCT_NAME}"` : PRODUCT_NAME
    const lines = [
        '[Desktop Entry]',
        'Categories=Game;',
        ...(config.bundle?.shortDescription
            ? [`Comment=${config.bundle.shortDescription}`]
            : []),
        `Exec=${exec} %U`,
        `StartupWMClass=${PRODUCT_NAME}`,
        `Icon=${PRODUCT_NAME}`,
        `Name=${PRODUCT_NAME}`,
        'Terminal=false',
        'Type=Application',
        ...(mimeTypes.length > 0
            ? [`MimeType=${mimeTypes.join(';')};`]
            : []),
        '',
    ]
    return lines.join('\n')
}

function controlFile(config, architecture, packageVersion, installedBytes) {
    const dependencies = config.bundle?.linux?.deb?.depends ?? []
    return [
        `Package: ${PACKAGE_NAME}`,
        `Version: ${packageVersion}`,
        `Architecture: ${debArchitecture(architecture)}`,
        `Installed-Size: ${Math.max(1, Math.ceil(installedBytes / 1024))}`,
        `Maintainer: Axolotl Launcher <support@axlmc.org>`,
        `Section: games`,
        'Priority: optional',
        `Homepage: ${HOMEPAGE}`,
        ...(dependencies.length > 0 ? [`Depends: ${dependencies.join(', ')}`] : []),
        `Recommends: ${RECOMMENDS}`,
        `Description: ${DESCRIPTION}`,
        '',
    ].join('\n')
}

const POSTINST = `#!/bin/sh
set -e

if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database -q /usr/share/applications || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -q -t -f /usr/share/icons/hicolor || true
fi
if command -v update-mime-database >/dev/null 2>&1; then
    update-mime-database /usr/share/mime || true
fi

exit 0
`

function runTar(archive, directory) {
    run('tar', [
        '--sort=name',
        '--mtime=@0',
        '--owner=0',
        '--group=0',
        '--numeric-owner',
        '-czf',
        archive,
        '-C',
        directory,
        '.',
    ])
}

function createDeb(workDirectory, output, architecture, config, packageVersion) {
    const dataDirectory = path.join(workDirectory, 'data')
    const controlDirectory = path.join(workDirectory, 'control')
    const archiveDirectory = path.join(workDirectory, 'deb')

    for (const directory of [dataDirectory, controlDirectory, archiveDirectory])
        fs.mkdirSync(directory, { recursive: true })

    const binary = path.join(dataDirectory, 'usr/bin', PRODUCT_NAME)
    fs.mkdirSync(path.dirname(binary), { recursive: true })
    fs.copyFileSync(requireFile(path.join(workDirectory, 'input')), binary)
    fs.chmodSync(binary, 0o755)

    const resources = config.bundle?.resources ?? []
    for (const resource of resources) {
        if (typeof resource !== 'string' || path.isAbsolute(resource))
            throw new Error(`Unsupported bundle resource path: ${resource}`)
        const source = path.join(workDirectory, 'resources', resource)
        const destination = path.join(
            dataDirectory,
            'usr/lib',
            PRODUCT_NAME,
            resource,
        )
        if (!fs.existsSync(source)) throw new Error(`Missing bundle resource: ${source}`)
        copyTree(source, destination)
    }

    const icons = (config.bundle?.icon ?? []).filter((icon) =>
        String(icon).toLowerCase().endsWith('.png'),
    )
    for (const icon of icons) {
        const source = path.join(workDirectory, 'icons', icon)
        const { width, height } = pngDimensions(requireFile(source))
        const highDensity = /@2x\.png$/i.test(icon)
        const destination = path.join(
            dataDirectory,
            'usr/share/icons/hicolor',
            `${width}x${height}${highDensity ? '@2' : ''}`,
            'apps',
            `${PRODUCT_NAME}.png`,
        )
        writeFile(destination, fs.readFileSync(source))
    }

    writeFile(
        path.join(
            dataDirectory,
            'usr/share/applications',
            `${PRODUCT_NAME}.desktop`,
        ),
        desktopEntry(config),
    )

    const dataFiles = files(dataDirectory)
    const checksums = dataFiles.map((file) => {
        const relative = path.relative(dataDirectory, file).replaceAll('\\', '/')
        return `${md5(file)}  ${relative}`
    })
    const installedBytes = dataFiles.reduce((total, file) => total + fs.statSync(file).size, 0)

    writeFile(path.join(controlDirectory, 'control'), controlFile(
        config,
        architecture,
        packageVersion,
        installedBytes,
    ))
    writeFile(path.join(controlDirectory, 'md5sums'), `${checksums.join('\n')}\n`)
    writeFile(path.join(controlDirectory, 'postinst'), POSTINST, 0o755)

    const controlArchive = path.join(archiveDirectory, 'control.tar.gz')
    const dataArchive = path.join(archiveDirectory, 'data.tar.gz')
    runTar(controlArchive, controlDirectory)
    runTar(dataArchive, dataDirectory)
    writeFile(path.join(archiveDirectory, 'debian-binary'), '2.0\n')

    const deb = path.join(
        archiveDirectory,
        `Axolotl.Launcher_${packageVersion}_${debArchitecture(architecture)}.deb`,
    )
    fs.rmSync(deb, { force: true })
    run(
        'ar',
        ['rc', deb, 'debian-binary', 'control.tar.gz', 'data.tar.gz'],
        { cwd: archiveDirectory },
    )
    fs.copyFileSync(deb, output)
    return output
}

function createPortable(workDirectory, output, architecture, packageVersion) {
    const portableDirectory = path.join(workDirectory, 'portable')
    const applicationDirectory = path.join(portableDirectory, PACKAGE_NAME)
    fs.mkdirSync(applicationDirectory, { recursive: true })
    copyTree(
        path.join(workDirectory, 'data/usr/bin'),
        path.join(applicationDirectory, 'bin'),
    )
    copyTree(
        path.join(workDirectory, 'data/usr/lib'),
        path.join(applicationDirectory, 'lib'),
    )
    fs.chmodSync(path.join(applicationDirectory, 'bin', PRODUCT_NAME), 0o755)
    writeFile(
        path.join(applicationDirectory, 'README.txt'),
        [
            `Axolotl Launcher ${packageVersion} portable (${architecture})`,
            '',
            'Run:',
            `  ./bin/${PRODUCT_NAME}`,
            '',
            'Keep bin and lib together so the launcher can find its resources.',
            'Java is not bundled. Install a JDK with your system package manager,',
            'then select it in the launcher settings.',
            '',
        ].join('\n'),
    )
    run('tar', [
        '--sort=name',
        '--mtime=@0',
        '--owner=0',
        '--group=0',
        '--numeric-owner',
        '-czf',
        output,
        '-C',
        portableDirectory,
        PACKAGE_NAME,
    ])
    return output
}

export function buildLinuxPackages({
    architecture,
    profile = 'ci',
    target,
    output = path.join(root, 'target/release-assets/linux'),
    targetRoot = path.join(root, 'target'),
    resourceRoot = app,
    iconRoot = app,
    config,
    packageVersion = version(),
    workRoot = path.join(root, 'target/linux-package'),
    binaryPath,
    resourcePaths,
    iconPaths,
}) {
    if (process.platform !== 'linux') throw new Error('Linux packaging requires Linux')
    const normalized = architecture
        ? normalizeArchitecture(architecture)
        : architectureFromTarget(target)
    if (target) {
        const targetArchitecture = architectureFromTarget(target)
        if (targetArchitecture !== normalized)
            throw new Error(`Target ${target} does not match architecture ${normalized}`)
    }
    config ??= JSON.parse(fs.readFileSync(path.join(app, 'tauri.conf.json'), 'utf8'))
    const binary =
        binaryPath ??
        path.join(
            targetRoot,
            ...(target ? [target] : []),
            profile,
            PRODUCT_NAME,
        )
    requireFile(binary)
    const binaryArchitecture = elfArchitecture(binary)
    if (binaryArchitecture !== normalized) {
        throw new Error(
            `Expected ${normalized} executable, found ${binaryArchitecture}: ${binary}`,
        )
    }

    const destination = path.resolve(root, output)
    fs.mkdirSync(destination, { recursive: true })
    const workDirectory = path.join(workRoot, normalized)
    fs.rmSync(workDirectory, { recursive: true, force: true })
    fs.mkdirSync(workDirectory, { recursive: true })
    fs.copyFileSync(binary, path.join(workDirectory, 'input'))

    for (const resource of resourcePaths ?? config.bundle?.resources ?? []) {
        if (typeof resource !== 'string' || path.isAbsolute(resource))
            throw new Error(`Unsupported bundle resource path: ${resource}`)
        const source = path.join(resourceRoot, resource)
        if (!fs.existsSync(source)) throw new Error(`Missing bundle resource: ${source}`)
        copyTree(source, path.join(workDirectory, 'resources', resource))
    }
    for (const icon of iconPaths ?? config.bundle?.icon ?? []) {
        if (!String(icon).toLowerCase().endsWith('.png')) continue
        const source = path.join(iconRoot, icon)
        requireFile(source)
        writeFile(
            path.join(workDirectory, 'icons', icon),
            fs.readFileSync(source),
        )
    }

    const deb = path.join(
        destination,
        `Axolotl.Launcher_${packageVersion}_${debArchitecture(normalized)}.deb`,
    )
    const portable = path.join(
        destination,
        `Axolotl_Launcher_${packageVersion}_${normalized}_portable.tar.gz`,
    )
    createDeb(workDirectory, deb, normalized, config, packageVersion)
    createPortable(workDirectory, portable, normalized, packageVersion)
    return { architecture: normalized, binarySha256: sha256(binary), deb, portable }
}

function main() {
    const { values } = parseArgs({
        options: {
            arch: { type: 'string' },
            target: { type: 'string' },
            profile: { type: 'string', default: 'release' },
            output: { type: 'string', default: path.join(root, 'target/release-assets/linux') },
        },
    })
    requireProfile(values.profile)
    if (values.arch && values.target)
        throw new Error('Specify --arch or --target, not both')
    if (!values.arch && !values.target) throw new Error('Expected --arch or --target')
    const result = buildLinuxPackages({
        architecture: values.arch,
        target: values.target,
        profile: values.profile,
        output: values.output,
    })
    console.log(`Packaged ${result.architecture}:`)
    console.log(result.deb)
    console.log(result.portable)
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) main()
