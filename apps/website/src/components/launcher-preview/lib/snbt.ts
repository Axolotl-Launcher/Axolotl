/**
 * SNBT reading for the command generator. The desktop app reads SNBT with
 * deepslate 0.26 (`NbtTag.fromString`, `NbtType`, `StringReader`); the site
 * cannot add that dependency, so this module reads the same syntax into tags
 * that answer the same queries, with the same string forms and error messages.
 */
export enum NbtType {
    End = 0,
    Byte = 1,
    Short = 2,
    Int = 3,
    Long = 4,
    Float = 5,
    Double = 6,
    ByteArray = 7,
    String = 8,
    List = 9,
    Compound = 10,
    IntArray = 11,
    LongArray = 12,
}

export class StringReader {
    cursor = 0

    constructor(readonly source: string) {}

    getRead(start = 0) {
        return this.source.substring(start, this.cursor)
    }

    canRead(length = 1) {
        return this.cursor + length <= this.source.length
    }

    peek(offset = 0) {
        return this.source.charAt(this.cursor + offset)
    }

    read() {
        return this.source.charAt(this.cursor++)
    }

    skip() {
        this.cursor += 1
    }

    skipWhitespace() {
        while (this.canRead() && StringReader.isWhitespace(this.peek())) this.skip()
    }

    expect(c: string, skipWhitespace = false) {
        if (skipWhitespace) this.skipWhitespace()
        if (!this.canRead() || this.peek() !== c) throw this.createError(`Expected '${c}'`)
        this.skip()
    }

    readUnquotedString() {
        const start = this.cursor
        while (this.canRead() && StringReader.isAllowedInUnquotedString(this.peek())) this.skip()
        return this.getRead(start)
    }

    readQuotedString() {
        if (!this.canRead()) return ''
        const c = this.peek()
        if (!StringReader.isQuotedStringStart(c)) {
            throw this.createError('Expected quote to start a string')
        }
        this.skip()
        return this.readStringUntil(c)
    }

    readString() {
        if (!this.canRead()) return ''
        const c = this.peek()
        if (StringReader.isQuotedStringStart(c)) {
            this.skip()
            return this.readStringUntil(c)
        }
        return this.readUnquotedString()
    }

    readStringUntil(terminator: string) {
        let result = ''
        let escaped = false
        while (this.canRead()) {
            const c = this.read()
            if (escaped) {
                if (c !== terminator && c !== '\\') {
                    this.cursor -= 1
                    throw this.createError(`Invalid escape sequence '${c}' in quoted string`)
                }
                result += c
                escaped = false
            } else if (c === '\\') {
                escaped = true
            } else if (c === terminator) {
                return result
            } else {
                result += c
            }
        }
        throw this.createError('Unclosed quoted string')
    }

    static isAllowedInUnquotedString(c: string) {
        return /^[0-9A-Za-z_.+-]$/.test(c)
    }

    static isQuotedStringStart(c: string) {
        return c === "'" || c === '"'
    }

    static isWhitespace(c: string) {
        return c === ' ' || c === '\t' || c === '\n' || c === '\r'
    }

    createError(message: string) {
        const cursor = Math.min(this.source.length, this.cursor)
        const context =
            (cursor > 10 ? '...' : '') + this.source.substring(Math.max(0, cursor - 10), cursor)
        return new Error(`${message} at position ${this.cursor}: ${context}<--[HERE]`)
    }
}

type TagValue = number | bigint | string | NbtTag[] | Map<string, NbtTag>

export class NbtTag {
    private constructor(
        private readonly id: NbtType,
        private readonly value: TagValue,
        /** Element type of a list. */
        private readonly itemType: NbtType = NbtType.End,
    ) {}

    getId() {
        return this.id
    }

    isCompound() {
        return this.id === NbtType.Compound
    }

    isList() {
        return this.id === NbtType.List
    }

    isString() {
        return this.id === NbtType.String
    }

    isLong() {
        return this.id === NbtType.Long
    }

    isLongArray() {
        return this.id === NbtType.LongArray
    }

    keys(): IterableIterator<string> {
        return this.value instanceof Map ? this.value.keys() : new Map<string, NbtTag>().keys()
    }

    get(key: string): NbtTag | undefined
    get(index: number): NbtTag
    get(key: string | number) {
        if (this.value instanceof Map) return this.value.get(String(key))
        return Array.isArray(this.value) ? this.value[Number(key)] : undefined
    }

    get length() {
        return Array.isArray(this.value) ? this.value.length : 0
    }

    getAsString() {
        return typeof this.value === 'string' ? this.value : ''
    }

    toString(): string {
        const value = this.value
        switch (this.id) {
            case NbtType.Byte:
                return (value as number).toFixed() + 'b'
            case NbtType.Short:
                return (value as number).toFixed() + 's'
            case NbtType.Int:
                return (value as number).toFixed()
            case NbtType.Long:
                return `${value as bigint}L`
            case NbtType.Float:
                return (value as number).toString() + 'f'
            case NbtType.Double:
                return Number.isInteger(value) ? (value as number).toFixed(1) : String(value)
            case NbtType.String:
                return '"' + (value as string).replace(/(\\|")/g, '\\$1') + '"'
            case NbtType.ByteArray:
                return `[B;${(value as NbtTag[]).map((item) => (item.value as number).toFixed() + 'B').join(',')}]`
            case NbtType.IntArray:
                return `[I;${(value as NbtTag[]).map((item) => (item.value as number).toFixed()).join(',')}]`
            case NbtType.LongArray:
                // deepslate 0.26 prints long arrays with the int array prefix.
                return `[I;${(value as NbtTag[]).map((item) => item.toString()).join(',')}]`
            case NbtType.List:
                return `[${(value as NbtTag[]).map((item) => item.toString()).join(',')}]`
            case NbtType.Compound:
                return `{${[...(value as Map<string, NbtTag>)]
                    .map(([key, item]) => {
                        const quoted = [...key].some(
                            (c) => !StringReader.isAllowedInUnquotedString(c),
                        )
                        return `${quoted ? JSON.stringify(key) : key}:${item.toString()}`
                    })
                    .join(',')}}`
            default:
                return 'END'
        }
    }

    /** `NbtTag.fromString`: one tag of any type, read from the reader's position. */
    static fromString(input: string | StringReader): NbtTag {
        return readTag(typeof input === 'string' ? new StringReader(input) : input)
    }

    static of(id: NbtType, value: TagValue, itemType?: NbtType) {
        return new NbtTag(id, value, itemType)
    }
}

const DOUBLE_NO_SUFFIX = /^[-+]?(?:[0-9]+[.]|[0-9]*[.][0-9]+)(?:e[-+]?[0-9]+)?$/i
const DOUBLE = /^[-+]?(?:[0-9]+[.]?|[0-9]*[.][0-9]+)(?:e[-+]?[0-9]+)?d$/i
const FLOAT = /^[-+]?(?:[0-9]+[.]?|[0-9]*[.][0-9]+)(?:e[-+]?[0-9]+)?f$/i
const BYTE = /^[-+]?(?:0|[1-9][0-9]*)b$/i
const LONG = /^[-+]?(?:0|[1-9][0-9]*)l$/i
const SHORT = /^[-+]?(?:0|[1-9][0-9]*)s$/i
const INT = /^[-+]?(?:0|[1-9][0-9]*)$/i

function readTag(reader: StringReader): NbtTag {
    reader.skipWhitespace()
    if (!reader.canRead()) throw reader.createError('Expected value')
    const c = reader.peek()
    if (c === '{') return readCompound(reader)
    if (c === '[') {
        if (
            reader.canRead(3) &&
            !StringReader.isQuotedStringStart(reader.peek(1)) &&
            reader.peek(2) === ';'
        ) {
            reader.expect('[', true)
            const start = reader.cursor
            const kind = reader.read()
            reader.skip()
            reader.skipWhitespace()
            if (!reader.canRead()) throw reader.createError('Expected value')
            if (kind === 'B') return readArray(reader, NbtType.ByteArray, NbtType.Byte)
            if (kind === 'L') return readArray(reader, NbtType.LongArray, NbtType.Long)
            if (kind === 'I') return readArray(reader, NbtType.IntArray, NbtType.Int)
            reader.cursor = start
            throw reader.createError(`Invalid array type '${kind}'`)
        }
        return readList(reader)
    }
    reader.skipWhitespace()
    const start = reader.cursor
    if (StringReader.isQuotedStringStart(reader.peek())) {
        return NbtTag.of(NbtType.String, reader.readQuotedString())
    }
    const value = reader.readUnquotedString()
    if (value.length === 0) {
        reader.cursor = start
        throw reader.createError('Expected value')
    }
    try {
        const body = value.substring(0, value.length - 1)
        if (FLOAT.test(value)) return NbtTag.of(NbtType.Float, Number(body))
        if (BYTE.test(value)) return NbtTag.of(NbtType.Byte, Math.floor(Number(body)))
        if (LONG.test(value)) return NbtTag.of(NbtType.Long, BigInt(body))
        if (SHORT.test(value)) return NbtTag.of(NbtType.Short, Math.floor(Number(body)))
        if (INT.test(value)) return NbtTag.of(NbtType.Int, Math.floor(Number(value)))
        if (DOUBLE.test(value)) return NbtTag.of(NbtType.Double, Number(body))
        if (DOUBLE_NO_SUFFIX.test(value)) return NbtTag.of(NbtType.Double, Number(value))
        if (value.toLowerCase() === 'true') return NbtTag.of(NbtType.Byte, 1)
        if (value.toLowerCase() === 'false') return NbtTag.of(NbtType.Byte, 0)
    } catch {
        // Unparsable numbers read as strings, as in deepslate.
    }
    return NbtTag.of(NbtType.String, value)
}

function hasElementSeparator(reader: StringReader) {
    reader.skipWhitespace()
    if (reader.canRead() && reader.peek() === ',') {
        reader.skip()
        reader.skipWhitespace()
        return true
    }
    return false
}

function readCompound(reader: StringReader) {
    reader.expect('{', true)
    const properties = new Map<string, NbtTag>()
    reader.skipWhitespace()
    while (reader.canRead() && reader.peek() !== '}') {
        const start = reader.cursor
        reader.skipWhitespace()
        if (!reader.canRead()) throw reader.createError('Expected key')
        const key = reader.readString()
        if (key.length === 0) {
            reader.cursor = start
            throw reader.createError('Expected key')
        }
        reader.expect(':', true)
        properties.set(key, readTag(reader))
        if (!hasElementSeparator(reader)) break
        if (!reader.canRead()) throw reader.createError('Expected key')
    }
    reader.expect('}', true)
    return NbtTag.of(NbtType.Compound, properties)
}

function readList(reader: StringReader) {
    reader.expect('[', true)
    reader.skipWhitespace()
    if (!reader.canRead()) throw reader.createError('Expected value')
    const items: NbtTag[] = []
    let type = NbtType.End
    while (reader.peek() !== ']') {
        const start = reader.cursor
        const value = readTag(reader)
        if (type === NbtType.End) {
            type = value.getId()
        } else if (value.getId() !== type) {
            reader.cursor = start
            throw reader.createError(
                `Can't insert ${NbtType[value.getId()]} into list of ${NbtType[type]}`,
            )
        }
        items.push(value)
        if (!hasElementSeparator(reader)) break
        if (!reader.canRead()) throw reader.createError('Expected value')
    }
    reader.expect(']', true)
    return NbtTag.of(NbtType.List, items, type)
}

function readArray(reader: StringReader, arrayId: NbtType, childId: NbtType) {
    const items: NbtTag[] = []
    while (reader.peek() !== ']') {
        const entry = readTag(reader)
        if (entry.getId() !== childId) {
            throw reader.createError(
                `Can't insert ${NbtType[entry.getId()]} into ${NbtType[arrayId]}`,
            )
        }
        items.push(entry)
        if (!hasElementSeparator(reader)) break
        if (!reader.canRead()) throw reader.createError('Expected value')
    }
    reader.expect(']')
    return NbtTag.of(arrayId, items)
}
