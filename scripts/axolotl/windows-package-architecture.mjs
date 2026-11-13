const machines = { x64: 0x8664, arm64: 0xaa64 }

export function windowsPackageArchitecture(host) {
    const architecture = {
        'x86_64-pc-windows-msvc': 'x64',
        'aarch64-pc-windows-msvc': 'arm64',
    }[host]
    if (!architecture) throw new Error(`Unsupported Windows packaging host: ${host}`)
    return architecture
}

export function verifyWindowsBinaryArchitecture(bytes, architecture, label) {
    if (!Object.hasOwn(machines, architecture))
        throw new Error(`Unsupported Windows package architecture: ${architecture}`)
    if (bytes.length < 64 || bytes.readUInt16LE(0) !== 0x5a4d)
        throw new Error(`Invalid Windows binary: ${label}`)
    const offset = bytes.readUInt32LE(0x3c)
    if (offset < 64 || offset > bytes.length - 6 || bytes.readUInt32LE(offset) !== 0x00004550)
        throw new Error(`Invalid PE header: ${label}`)
    if (bytes.readUInt16LE(offset + 4) !== machines[architecture])
        throw new Error(`Windows binary architecture does not match ${architecture}: ${label}`)
}

export function verifyWindowsBundleArchitecture(filename, architecture) {
    if (
        !Object.hasOwn(machines, architecture) ||
        ![`.exe`, `.nsis.zip`].some((extension) =>
            filename.endsWith(`_${architecture}-setup${extension}`),
        )
    )
        throw new Error(`Windows bundle architecture does not match ${architecture}: ${filename}`)
}
