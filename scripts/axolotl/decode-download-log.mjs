import fs from 'node:fs/promises'

const [file] = process.argv.slice(2)

const input = file
    ? await fs.readFile(file, 'utf8')
    : await new Promise((resolve, reject) => {
        let data = ''
        process.stdin.setEncoding('utf8')
        process.stdin.on('data', (chunk) => {
            data += chunk
        })
        process.stdin.on('end', () => resolve(data))
        process.stdin.on('error', reject)
    })

const engines = { 0: 'legacy', 1: 'xmcl' }
const rules = {
    1: 'R1NoProgress',
    2: 'R2BelowExpectation',
    3: 'R3SegmentWaste',
    4: 'R4FrequentSwitches',
}
const sources = {
    0: 'official',
    1: 'bmclapi',
    2: 'mcim',
    3: 'alternate',
    4: 'unknown',
    5: 'tianpao',
}

for (const line of input.split('\n')) {
    if (!line) continue
    const [timestamp, engine, rule, source, ...detail] = line.split('|')
    const date = new Date(Number(timestamp) * 1000).toISOString()
    console.log(
        `${date} engine=${engines[engine] ?? engine} rule=${rules[rule] ?? rule} source=${sources[source] ?? source} ${detail.join('|')}`,
    )
}
