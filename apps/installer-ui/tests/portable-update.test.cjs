const assert = require('node:assert/strict')
const fs = require('node:fs')
const os = require('node:os')
const path = require('node:path')
const { createHash } = require('node:crypto')
const { execFileSync, spawn } = require('node:child_process')
const { once } = require('node:events')
const { after, test } = require('node:test')
const { setTimeout: delay } = require('node:timers/promises')

const root = path.resolve(__dirname, '../../..')
const windows = process.platform === 'win32'
const helper = process.env.AXOLOTL_INSTALLER_UI_PATH
    ? path.normalize(process.env.AXOLOTL_INSTALLER_UI_PATH)
    : ['ci', 'debug', 'release']
          .map((profile) => path.join(root, 'target', profile, 'axolotl-installer-ui.exe'))
          .find((candidate) => fs.existsSync(candidate))
const compiler = path.join(process.env.LOCALAPPDATA || '', 'tauri/NSIS/makensis.exe')
const directory = windows
    ? fs.mkdtempSync(path.join(os.tmpdir(), 'axolotl-portable-worker-'))
    : null

after(async () => {
    if (directory)
        await until(() => {
            try {
                fs.rmSync(directory, { recursive: true, force: true })
                return true
            } catch (error) {
                if (!['EPERM', 'EBUSY', 'ENOTEMPTY'].includes(error.code)) throw error
                return false
            }
        })
})

function digest(bytes) {
    return { size: bytes.length, sha256: createHash('sha256').update(bytes).digest('hex') }
}

async function until(predicate) {
    const deadline = Date.now() + 20_000
    while (!predicate()) {
        assert.ok(Date.now() < deadline, 'Timed out waiting for the portable worker fixture')
        await delay(10)
    }
}

async function completion(child) {
    if (child.exitCode !== null || child.signalCode !== null) return child.exitCode
    return (await once(child, 'exit'))[0]
}

function setup(label) {
    assert.ok(helper && fs.existsSync(helper), 'Build axolotl-installer-ui first')
    assert.ok(fs.existsSync(compiler), 'Install Tauri NSIS first')
    const target = path.join(directory, `${label} 中文 portable`)
    const stage = path.join(target, '.axolotl-update-fixture')
    fs.mkdirSync(path.join(stage, 'payload'), { recursive: true })
    fs.mkdirSync(path.join(target, '.Axolotl'))
    fs.writeFileSync(path.join(target, '.Axolotl/app.db'), 'keep database')
    fs.writeFileSync(path.join(target, 'user.txt'), 'keep user file')
    const marker = path.join(target, 'restarted')
    const next = path.join(directory, `${label}.exe`)
    const script = path.join(directory, `${label}.nsi`)
    fs.writeFileSync(
        script,
        [
            'Unicode true',
            'RequestExecutionLevel user',
            'SilentInstall silent',
            'AutoCloseWindow true',
            'Name "Portable restart fixture"',
            `OutFile "${next}"`,
            'Section',
            `FileOpen $0 "${marker}" w`,
            'FileWrite $0 "new executable"',
            'FileClose $0',
            'SectionEnd',
        ].join('\n'),
    )
    execFileSync(compiler, ['/INPUTCHARSET', 'UTF8', '/V2', script], { windowsHide: true })
    const executable = path.join(target, 'Renamed Launcher.exe')
    fs.copyFileSync(helper, executable)
    const original = fs.readFileSync(executable)
    const files = [
        ['Axolotl Launcher.exe', fs.readFileSync(next)],
        ['axolotl-updater.exe', fs.readFileSync(helper)],
        ['resources/blockbench-skin/index.html', Buffer.from('new resources')],
        ['resources/seed-map/axolotl-cubiomes.dll', Buffer.from('fixture native resource')],
    ]
    const inventory = files.map(([file, bytes]) => {
        const destination = path.join(stage, 'payload', file)
        fs.mkdirSync(path.dirname(destination), { recursive: true })
        fs.writeFileSync(destination, bytes)
        return { path: file, next: digest(bytes), previous: null }
    })
    fs.writeFileSync(path.join(stage, 'inventory.json'), JSON.stringify(inventory))
    fs.copyFileSync(helper, path.join(stage, 'worker.exe'))
    return { target, stage, marker, executable, original, next: fs.readFileSync(next) }
}

for (const restart of [false, true]) {
    test(
        `native portable worker waits for exit and ${restart ? 'restarts new executable' : 'quits'}`,
        { skip: !windows },
        async () => {
            const fixture = setup(restart ? 'restart' : 'quit')
            const parent = spawn(process.execPath, ['-e', 'setInterval(() => {}, 1000)'], {
                windowsHide: true,
            })
            const worker = spawn(
                path.join(fixture.stage, 'worker.exe'),
                [
                    '--portable-update',
                    fixture.stage,
                    fixture.executable,
                    String(parent.pid),
                    restart ? 'restart' : 'quit',
                ],
                { windowsHide: true },
            )
            const workerDone = completion(worker)
            try {
                await until(
                    () =>
                        fs.existsSync(path.join(fixture.stage, 'ready')) ||
                        worker.exitCode !== null,
                )
                assert.equal(
                    worker.exitCode,
                    null,
                    fs.existsSync(path.join(fixture.stage, 'error.txt'))
                        ? fs.readFileSync(path.join(fixture.stage, 'error.txt'), 'utf8')
                        : 'worker exited early',
                )
                assert.deepEqual(fs.readFileSync(fixture.executable), fixture.original)
                assert.equal(fs.existsSync(fixture.marker), false)
                parent.kill()
                await completion(parent)
                assert.equal(await workerDone, 0)
                assert.deepEqual(fs.readFileSync(fixture.executable), fixture.next)
                if (restart) await until(() => fs.existsSync(fixture.marker))
                else assert.equal(fs.existsSync(fixture.marker), false)
                assert.equal(
                    fs.readFileSync(path.join(fixture.target, '.Axolotl/app.db'), 'utf8'),
                    'keep database',
                )
                assert.equal(
                    fs.readFileSync(path.join(fixture.target, 'user.txt'), 'utf8'),
                    'keep user file',
                )
                assert.equal(
                    fs.existsSync(path.join(fixture.target, '.axolotl-portable-update.json')),
                    false,
                )
            } finally {
                if (parent.exitCode === null) parent.kill()
                if (worker.exitCode === null) worker.kill()
                await Promise.all([completion(parent), workerDone])
            }
        },
    )
}
