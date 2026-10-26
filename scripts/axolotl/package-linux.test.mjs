import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import fs from 'node:fs'
import path from 'node:path'
import test from 'node:test'

import {
    architectures,
    architectureFromTarget,
    buildLinuxPackages,
    debArchitecture,
    elfArchitecture,
    normalizeArchitecture,
} from './package-linux.mjs'
import { app } from './desktop-build-utils.mjs'

const root = '/tmp/opencode'

function temporaryDirectory() {
    fs.mkdirSync(root, { recursive: true })
    return fs.mkdtempSync(path.join(root, 'axolotl-package-'))
}

function fakeElf(machine) {
    const header = Buffer.alloc(64)
    header.set([0x7f, 0x45, 0x4c, 0x46, 2, 1, 1], 0)
    header.writeUInt16LE(2, 16)
    header.writeUInt16LE(machine, 18)
    return header
}

function execute(command, args, options = {}) {
    const result = spawnSync(command, args, {
        encoding: 'utf8',
        stdio: ['ignore', 'pipe', 'pipe'],
        ...options,
    })
    assert.equal(result.status, 0, result.stderr || result.stdout)
    return result.stdout
}

function extractMember(deb, member, destination) {
    const result = spawnSync('ar', ['p', deb, member], { encoding: null })
    assert.equal(result.status, 0, result.stderr?.toString())
    const archive = path.join(path.dirname(destination), `${member}.tar.gz`)
    fs.writeFileSync(archive, result.stdout)
    fs.mkdirSync(destination, { recursive: true })
    execute('tar', ['-xzf', archive, '-C', destination])
}

function fixture(directory) {
    const targetRoot = path.join(directory, 'target')
    const binary = path.join(targetRoot, 'ci', 'Axolotl Launcher')
    fs.mkdirSync(path.dirname(binary), { recursive: true })
    fs.writeFileSync(binary, fakeElf(architectures.loongarch64.elf))

    const resourceRoot = path.join(directory, 'app')
    const resources = path.join(resourceRoot, 'resources/blockbench-skin')
    fs.mkdirSync(path.join(resources, 'dist'), { recursive: true })
    fs.writeFileSync(path.join(resources, 'dist/skin.bundle.js.gz'), 'fake bundle')
    fs.writeFileSync(path.join(resources, 'index.html'), '<!doctype html>')

    return {
        targetRoot,
        resourceRoot,
        config: {
            bundle: {
                resources: ['resources/blockbench-skin'],
                icon: ['icons/128x128.png', 'icons/128x128@2x.png'],
                linux: {
                    deb: {
                        depends: ['libwebkit2gtk-4.1-0', 'libgtk-3-0'],
                    },
                },
                fileAssociations: [
                    {
                        mimeType: 'application/x-modrinth-modpack+zip',
                        extensions: ['mrpack'],
                    },
                ],
            },
            plugins: {
                'deep-link': {
                    desktop: {
                        schemes: ['axolotl'],
                    },
                },
            },
        },
    }
}

test('normalizes package architectures and never maps an unknown one', () => {
    assert.equal(normalizeArchitecture('amd64'), 'x86_64')
    assert.equal(normalizeArchitecture('arm64'), 'aarch64')
    assert.equal(normalizeArchitecture('loong64'), 'loongarch64')
    assert.equal(normalizeArchitecture('loongarch64el'), 'loongarch64')
    assert.equal(normalizeArchitecture('riscv64gc'), 'riscv64')
    assert.equal(debArchitecture('loongarch64'), 'loong64')
    assert.equal(debArchitecture('riscv64'), 'riscv64')
    assert.equal(
        architectureFromTarget('loongarch64-unknown-linux-gnu'),
        'loongarch64',
    )
    assert.equal(
        architectureFromTarget('riscv64gc-unknown-linux-gnu'),
        'riscv64',
    )
    assert.throws(() => normalizeArchitecture('ppc64le'), /Unsupported/)
})

test('reads the architecture from a little-endian 64-bit ELF header', () => {
    const directory = temporaryDirectory()
    try {
        for (const [architecture, details] of Object.entries(architectures)) {
            const file = path.join(directory, architecture)
            fs.writeFileSync(file, fakeElf(details.elf))
            assert.equal(elfArchitecture(file), architecture)
        }
        const invalid = path.join(directory, 'invalid')
        fs.writeFileSync(invalid, Buffer.alloc(20, 0x78))
        assert.throws(() => elfArchitecture(invalid), /Expected a 64-bit/)
    } finally {
        fs.rmSync(directory, { recursive: true, force: true })
    }
})

for (const architecture of ['loongarch64', 'riscv64']) {
    test(
        `packages a ${architecture} deb and portable archive`,
        { skip: process.platform !== 'linux' },
        () => {
            const directory = temporaryDirectory()
            try {
                const setup = fixture(directory)
                const binary = path.join(setup.targetRoot, 'ci', 'Axolotl Launcher')
                fs.writeFileSync(binary, fakeElf(architectures[architecture].elf))
                const output = path.join(directory, 'output')
                const result = buildLinuxPackages({
                    architecture,
                    profile: 'ci',
                    targetRoot: setup.targetRoot,
                    resourceRoot: setup.resourceRoot,
                    iconRoot: app,
                    config: setup.config,
                    packageVersion: '1.2.3',
                    workRoot: path.join(directory, 'work'),
                    output,
                })

                const debArch = debArchitecture(architecture)
                assert.equal(result.architecture, architecture)
                assert.equal(result.binarySha256.length, 64)
                assert.deepEqual(
                    path.basename(result.deb),
                    `Axolotl.Launcher_1.2.3_${debArch}.deb`,
                )
                assert.deepEqual(
                    path.basename(result.portable),
                    `Axolotl_Launcher_1.2.3_${architecture}_portable.tar.gz`,
                )

                const members = execute('ar', ['t', result.deb]).trim().split('\n')
                assert.deepEqual(members, ['debian-binary', 'control.tar.gz', 'data.tar.gz'])

                const control = path.join(directory, 'control')
                extractMember(result.deb, 'control.tar.gz', control)
                const controlFile = fs.readFileSync(path.join(control, 'control'), 'utf8')
                assert.match(controlFile, /^Package: axolotl-launcher$/m)
                assert.match(controlFile, new RegExp(`^Architecture: ${debArch}$`, 'm'))
                assert.match(
                    controlFile,
                    /^Depends: libwebkit2gtk-4\.1-0, libgtk-3-0$/m,
                )
                assert.doesNotMatch(controlFile, /^Depends: .*openjdk/m)
                assert.match(controlFile, /^Recommends: .*openjdk-21-jre-headless/m)
                assert.match(controlFile, /^Homepage: https:\/\/axlmc\.org$/m)
                assert.equal(
                    fs.statSync(path.join(control, 'postinst')).mode & 0o777,
                    0o755,
                )
                const md5sums = fs.readFileSync(path.join(control, 'md5sums'), 'utf8')
                assert.match(md5sums, /^[a-f0-9]{32} {2}usr\/bin\/Axolotl Launcher$/m)

                const data = path.join(directory, 'data')
                extractMember(result.deb, 'data.tar.gz', data)
                const installedBinary = path.join(data, 'usr/bin/Axolotl Launcher')
                assert.equal(fs.statSync(installedBinary).mode & 0o777, 0o755)
                assert.equal(fs.readFileSync(installedBinary).subarray(0, 4).toString(), '\x7fELF')
                assert.ok(
                    fs.existsSync(
                        path.join(
                            data,
                            'usr/lib/Axolotl Launcher/resources/blockbench-skin/dist/skin.bundle.js.gz',
                        ),
                    ),
                )
                assert.ok(
                    fs.existsSync(
                        path.join(data, 'usr/share/icons/hicolor/128x128/apps/Axolotl Launcher.png'),
                    ),
                )
                assert.ok(
                    fs.existsSync(
                        path.join(
                            data,
                            'usr/share/icons/hicolor/256x256@2/apps/Axolotl Launcher.png',
                        ),
                    ),
                )
                const desktop = fs.readFileSync(
                    path.join(data, 'usr/share/applications/Axolotl Launcher.desktop'),
                    'utf8',
                )
                assert.match(desktop, /^Categories=Game;$/m)
                assert.match(desktop, /^Exec="Axolotl Launcher" %U$/m)
                assert.match(desktop, /^StartupWMClass=Axolotl Launcher$/m)
                assert.match(
                    desktop,
                    /^MimeType=application\/x-modrinth-modpack\+zip;x-scheme-handler\/axolotl;$/m,
                )

                const portableFiles = execute('tar', ['-tzf', result.portable])
                    .trim()
                    .split('\n')
                assert.ok(portableFiles.includes('axolotl-launcher/bin/Axolotl Launcher'))
                assert.ok(
                    portableFiles.includes(
                        'axolotl-launcher/lib/Axolotl Launcher/resources/blockbench-skin/dist/skin.bundle.js.gz',
                    ),
                )
                assert.ok(portableFiles.includes('axolotl-launcher/README.txt'))

                const portableRoot = path.join(directory, 'portable-extract')
                fs.mkdirSync(portableRoot)
                execute('tar', ['-xzf', result.portable, '-C', portableRoot])
                const portableBinary = path.join(
                    portableRoot,
                    'axolotl-launcher/bin/Axolotl Launcher',
                )
                assert.equal(
                    fs.statSync(portableBinary).mode & 0o777,
                    0o755,
                )
            } finally {
                fs.rmSync(directory, { recursive: true, force: true })
            }
        },
    )
}

test(
    'rejects a binary whose ELF architecture does not match the package',
    { skip: process.platform !== 'linux' },
    () => {
        const directory = temporaryDirectory()
        try {
            const setup = fixture(directory)
            const binary = path.join(setup.targetRoot, 'ci', 'Axolotl Launcher')
            fs.writeFileSync(binary, fakeElf(architectures.riscv64.elf))
            assert.throws(
                () =>
                    buildLinuxPackages({
                        architecture: 'loongarch64',
                        profile: 'ci',
                        targetRoot: setup.targetRoot,
                        resourceRoot: setup.resourceRoot,
                        iconRoot: app,
                        config: setup.config,
                        packageVersion: '1.2.3',
                        workRoot: path.join(directory, 'work'),
                        output: path.join(directory, 'output'),
                    }),
                /Expected loongarch64 executable, found riscv64/,
            )
        } finally {
            fs.rmSync(directory, { recursive: true, force: true })
        }
    },
)
