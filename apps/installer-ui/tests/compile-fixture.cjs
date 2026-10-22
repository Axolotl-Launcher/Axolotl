// Compile the real NSIS template with an isolated product, registry key and data directory.
// After a Tauri Windows build:
// npm install --prefix target/uninstall-verification --no-save handlebars@4.7.8
// node apps/installer-ui/tests/compile-fixture.cjs --tauri-nsis target/release/nsis/x64
const fs = require('node:fs')
const path = require('node:path')
const { execFileSync } = require('node:child_process')
const { createRequire } = require('node:module')
const { parseArgs } = require('node:util')
const { values: options } = parseArgs({
    options: {
        'tauri-nsis': { type: 'string' },
        nsis: { type: 'string' },
        template: { type: 'string' },
    },
})

const root = path.resolve(__dirname, '../../..')
const out = path.join(root, 'target/uninstall-verification')
if (!options['tauri-nsis'])
    throw new Error('Provide --tauri-nsis with Tauri NSIS includes and languages')
const tauri = path.resolve(options['tauri-nsis'])
const languageRoot = fs.existsSync(path.join(tauri, 'languages'))
    ? path.join(tauri, 'languages')
    : tauri
const nsis = options.nsis || path.join(process.env.LOCALAPPDATA, 'tauri/NSIS')
const fixtureRequire = createRequire(path.join(out, 'package.json'))
const Handlebars = fixtureRequire('handlebars')
fs.mkdirSync(out, { recursive: true })
Handlebars.registerHelper('no-escape', (value) => new Handlebars.SafeString(value))
Handlebars.registerHelper('or', (a, b) => a || b)
Handlebars.registerHelper('association-description', (a, b) => a || b)
fs.writeFileSync(path.join(out, 'fixture.exe'), 'fixture binary')
fs.writeFileSync(path.join(out, 'AxolotlUninstallFixture.exe'), 'fixture binary')
fs.writeFileSync(path.join(out, 'resource.txt'), 'fixture resource')
const hook = fs
    .readFileSync(path.join(root, 'apps/app/nsis/hooks.nsi'), 'utf8')
    .replace(
        /!define AXL_INSTALLER_UI_PATH .*/g,
        `!define AXL_INSTALLER_UI_PATH "${path.join(out, 'fixture.exe')}"`,
    )
fs.writeFileSync(path.join(out, 'hooks.nsi'), hook)
for (const file of ['utils.nsh', 'FileAssociation.nsh']) {
    fs.copyFileSync(path.join(tauri, file), path.join(out, file))
}
const values = {
    compression: 'none',
    manufacturer: 'AxolotlUninstallTest',
    product_name: 'Axolotl Uninstall Fixture',
    version: '1.0.0',
    version_with_build: '1.0.0.0',
    install_mode: 'currentUser',
    installer_icon: path.join(root, 'apps/app/icons/icon.ico'),
    uninstaller_icon: path.join(root, 'apps/app/icons/icon.ico'),
    main_binary_name: 'AxolotlUninstallFixture',
    main_binary_path: path.join(out, 'AxolotlUninstallFixture.exe'),
    bundle_id: 'red.ghs.axolotl.uninstall-fixture',
    out_file: path.join(out, 'setup.exe'),
    arch: 'x64',
    additional_plugins_path: path.join(nsis, 'Plugins/x86-unicode/additional'),
    allow_downgrades: 'true',
    display_language_selector: 'false',
    install_webview2_mode: 'skip',
    installer_hooks: path.join(out, 'hooks.nsi'),
    start_menu_folder: 'Axolotl Uninstall Fixture',
    languages: ['English', 'SimpChinese'],
    language_files: ['English.nsh', 'SimpChinese.nsh'].map((file) => path.join(languageRoot, file)),
    resources: { [path.join(out, 'resource.txt')]: ['fixture', 'resources\\resource.txt'] },
    resources_dirs: ['resources'],
    resources_ancestors: ['resources'],
    binaries: [],
    file_associations: [],
    deep_link_protocols: [],
}
const template = fs.readFileSync(
    options.template || path.join(root, 'apps/app/nsis/installer.nsi'),
    'utf8',
)
fs.writeFileSync(
    path.join(out, 'fixture.nsi'),
    Handlebars.compile(template, { noEscape: true })(values),
)
try {
    console.log(
        execFileSync(
            path.join(nsis, 'makensis.exe'),
            ['/INPUTCHARSET', 'UTF8', '/V2', path.join(out, 'fixture.nsi')],
            { encoding: 'utf8' },
        ),
    )
} catch (error) {
    console.error(error.stdout?.toString(), error.stderr?.toString())
    process.exit(1)
}
