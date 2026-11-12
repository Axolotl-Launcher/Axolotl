const assert = require('node:assert/strict')
const fs = require('node:fs')
const os = require('node:os')
const path = require('node:path')
const { execFileSync, spawn } = require('node:child_process')
const { once } = require('node:events')
const { after, test } = require('node:test')
const { setTimeout: delay } = require('node:timers/promises')

const root = path.resolve(__dirname, '../../..')
const windows = process.platform === 'win32'
const helper =
    process.env.AXOLOTL_INSTALLER_UI_PATH ||
    ['ci', 'debug', 'release']
        .map((profile) => path.join(root, 'target', profile, 'axolotl-installer-ui.exe'))
        .find((candidate) => fs.existsSync(candidate))
const compiler = path.join(process.env.LOCALAPPDATA || '', 'tauri/NSIS/makensis.exe')
const directory = windows
    ? fs.mkdtempSync(path.join(os.tmpdir(), 'axolotl-executable-test-'))
    : undefined
after(() => {
    if (directory) {
        assert.equal(path.dirname(directory), path.resolve(os.tmpdir()))
        fs.rmSync(directory, { recursive: true, force: true })
    }
})

function fixture(label, { source = helper, metadata = '', pause = false, crc = 'force' } = {}) {
    assert.ok(
        helper && fs.existsSync(helper),
        'Build axolotl-installer-ui before running native tests',
    )
    assert.ok(fs.existsSync(compiler), 'Install Tauri NSIS before running native tests')
    const installer = path.join(directory, `${label}.exe`)
    const script = path.join(directory, `${label}.nsi`)
    fs.writeFileSync(
        script,
        [
            'Unicode true',
            'RequestExecutionLevel user',
            'SilentInstall silent',
            'AutoCloseWindow true',
            `CRCCheck ${crc}`,
            'SetCompress off',
            '!include LogicLib.nsh',
            'Var StatusFile',
            `!define AXL_INSTALLER_UI_PATH "${helper}"`,
            `!include "${path.join(root, 'apps/app/nsis/executable-install.nsh')}"`,
            `!insertmacro AxlExecutableMetadata "${source}"`,
            metadata,
            'Name "Axolotl executable replacement fixture"',
            `OutFile "${installer}"`,
            `InstallDir "${directory}"`,
            'Section',
            'SetOutPath $INSTDIR',
            '!insertmacro AxlAcquireInstallOwnership',
            ...(pause ? ['FileOpen $1 "$INSTDIR\\lock-held" w', 'FileClose $1', 'Sleep 2000'] : []),
            `!insertmacro AxlInstallExecutable "${source}" "$INSTDIR\\launcher.exe"`,
            'SectionEnd',
        ].join('\n'),
    )
    execFileSync(compiler, ['/INPUTCHARSET', 'UTF8', '/V2', script], { windowsHide: true })
    return installer
}

function launch(installer, target) {
    return spawn(installer, ['/S', `/D=${target}`], { windowsHide: true })
}

async function completion(child) {
    if (child.exitCode !== null) return child.exitCode
    const [code] = await once(child, 'exit')
    return code
}

function installDirectory(label) {
    const destination = path.join(directory, label)
    fs.mkdirSync(destination)
    fs.writeFileSync(path.join(destination, 'launcher.exe'), `previous healthy ${label}`)
    return destination
}

async function until(predicate, milliseconds = 20_000) {
    const end = Date.now() + milliseconds
    while (!predicate()) {
        assert.ok(Date.now() < end, 'Timed out waiting for the owned installer fixture')
        await delay(2)
    }
}

test('both package variants use protected executable publication', () => {
    for (const variant of ['native', 'modern']) {
        const config = JSON.parse(
            fs.readFileSync(path.join(root, `apps/app/tauri-${variant}.conf.json`)),
        )
        assert.equal(config.bundle.windows.nsis.template, './nsis/installer.nsi')
    }
    const template = fs.readFileSync(path.join(root, 'apps/app/nsis/installer.nsi'), 'utf8')
    assert.match(template, /!insertmacro AxlInstallExecutable/)
    assert.doesNotMatch(template, /^\s*File "\$\{MAINBINARYSRCPATH\}"/m)
})

test('valid extraction replaces atomically and retains old bytes', { skip: !windows }, async () => {
    const installer = fixture('complete')
    const target = installDirectory('complete')
    const old = fs.readFileSync(path.join(target, 'launcher.exe'))
    assert.equal(await completion(launch(installer, target)), 0)
    assert.deepEqual(fs.readFileSync(path.join(target, 'launcher.exe')), fs.readFileSync(helper))
    const backup = fs.readdirSync(target).find((name) => name.startsWith('.axolotl-previous-'))
    assert.ok(backup)
    assert.deepEqual(fs.readFileSync(path.join(target, backup)), old)
})

test(
    'integrity rejection leaves the previous executable unchanged',
    { skip: !windows },
    async () => {
        const installer = fixture('bad-hash', {
            metadata:
                '!undef AXL_EXECUTABLE_SHA256\n!define AXL_EXECUTABLE_SHA256 "' +
                '0'.repeat(64) +
                '"',
        })
        const target = installDirectory('bad-hash')
        const original = fs.readFileSync(path.join(target, 'launcher.exe'))
        assert.notEqual(await completion(launch(installer, target)), 0)
        assert.deepEqual(fs.readFileSync(path.join(target, 'launcher.exe')), original)
    },
)

test(
    'first extraction failure cannot truncate the installed executable',
    { skip: !windows },
    async () => {
        // Disable only the fixture CRC to inject a fault after extraction opens its
        // output. Production signed-package validation is not bypassed by this test.
        const source = path.join(directory, 'invalid-compression-source.exe')
        fs.writeFileSync(
            source,
            Buffer.concat([
                fs.readFileSync(helper),
                Buffer.from('AXOLOTL_UNIQUE_EXTRACTION_FAULT'),
            ]),
        )
        const installer = fixture('extraction-failure', { source, crc: 'off' })
        const packageBytes = fs.readFileSync(installer)
        const sourceBytes = fs.readFileSync(source)
        const offset = packageBytes.indexOf(sourceBytes)
        assert.ok(offset >= 4, 'Expected the uncompressed executable data block')
        assert.equal(packageBytes.readUInt32LE(offset - 4), sourceBytes.length)
        packageBytes.writeUInt32LE((sourceBytes.length | 0x80000000) >>> 0, offset - 4)
        packageBytes[offset] = 7 // Invalid raw DEFLATE BTYPE=3 before any output.
        fs.writeFileSync(installer, packageBytes)
        const target = installDirectory('extraction-failure')
        const original = fs.readFileSync(path.join(target, 'launcher.exe'))
        assert.notEqual(await completion(launch(installer, target)), 0)
        assert.deepEqual(fs.readFileSync(path.join(target, 'launcher.exe')), original)
    },
)

test('a competing installer cannot mutate the owned installation', { skip: !windows }, async () => {
    const installer = fixture('exclusive', { pause: true })
    const target = installDirectory('exclusive')
    const original = fs.readFileSync(path.join(target, 'launcher.exe'))
    const first = launch(installer, target)
    try {
        await until(() => fs.existsSync(path.join(target, 'lock-held')))
        assert.notEqual(await completion(launch(installer, target)), 0)
        assert.deepEqual(fs.readFileSync(path.join(target, 'launcher.exe')), original)
    } finally {
        first.kill()
        await completion(first)
    }
    assert.equal(await completion(launch(installer, target)), 0)
})

test(
    'termination during real extraction preserves the installed executable',
    { skip: !windows },
    async () => {
        const large = path.join(directory, 'large.exe')
        fs.copyFileSync(helper, large)
        const handle = fs.openSync(large, 'a')
        try {
            const chunk = Buffer.alloc(1024 * 1024, 83)
            for (let count = 0; count < 128; count++) fs.writeSync(handle, chunk)
        } finally {
            fs.closeSync(handle)
        }
        const installer = fixture('interrupted', { source: large })
        const target = installDirectory('interrupted')
        const original = fs.readFileSync(path.join(target, 'launcher.exe'))
        const child = launch(installer, target)
        let interrupted = false
        try {
            await until(() => {
                if (child.exitCode !== null)
                    throw new Error('Installer completed before extraction interruption')
                const stage = fs.readdirSync(target).find((file) => file.endsWith('.tmp'))
                if (!stage) return false
                const size = fs.statSync(path.join(target, stage)).size
                return size > 0 && size < fs.statSync(large).size
            })
            child.kill()
            interrupted = true
        } finally {
            if (!interrupted) child.kill()
            await completion(child)
        }
        assert.deepEqual(fs.readFileSync(path.join(target, 'launcher.exe')), original)
    },
)
