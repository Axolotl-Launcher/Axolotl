#!/usr/bin/env node
import { spawn } from 'child_process'
import { fileURLToPath } from 'url'
import { dirname, join } from 'path'

const __dirname = dirname(fileURLToPath(import.meta.url))
const [scriptName, ...args] = process.argv.slice(2)

if (!scriptName) {
    console.error('Usage: pnpm scripts <script-name> [args...]')
    console.error('Example: pnpm scripts coverage-i18n --verbose')
    process.exit(1)
}

const scriptPath = join(__dirname, `${scriptName}.ts`)

// Keep arguments out of a shell so metacharacters cannot become commands.
const pnpx =
    process.platform === 'win32'
        ? join(dirname(process.execPath), 'node_modules', 'corepack', 'dist', 'pnpx.js')
        : 'pnpx'
const command = process.platform === 'win32' ? process.execPath : pnpx
const commandArgs =
    process.platform === 'win32' ? [pnpx, 'tsx', scriptPath, ...args] : ['tsx', scriptPath, ...args]
const child = spawn(command, commandArgs, {
    stdio: 'inherit',
})

child.on('exit', (code) => {
    process.exit(code ?? 0)
})
