import { execFileSync } from 'node:child_process'
import fs from 'node:fs'

const tag = process.argv[2]
const version = tag?.replace(/^v/, '')

if (!version || !/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/.test(version)) {
    throw new Error(`Expected a semantic version tag such as v1.2.3, received: ${tag ?? '<none>'}`)
}

const packagePath = 'apps/app-frontend/package.json'
const packageJson = JSON.parse(fs.readFileSync(packagePath, 'utf8'))
packageJson.version = version
fs.writeFileSync(packagePath, `${JSON.stringify(packageJson, null, 2)}\n`)

for (const cargoPath of ['apps/app/Cargo.toml', 'packages/app-lib/Cargo.toml']) {
    const cargoToml = fs.readFileSync(cargoPath, 'utf8')
    const packageVersionPattern = /^(\[package\][\s\S]*?^version\s*=\s*)"([^"]+)"/m
    const match = cargoToml.match(packageVersionPattern)

    if (!match) {
        throw new Error(`Could not find package version in ${cargoPath}`)
    }

    if (match[2] === version) {
        continue
    }

    const updated = cargoToml.replace(packageVersionPattern, `$1"${version}"`)
    fs.writeFileSync(cargoPath, updated)
}

// Refresh path-package versions while preserving locked third-party dependencies.
execFileSync('cargo', ['update', '--workspace'], { stdio: 'inherit' })

console.log(`Configured Axolotl Launcher ${version}`)
