import type { SeedMapMarker } from './workspace.ts'

type Rect = { x: number; y: number; width: number; height: number }
export type SeedMapMarkerPlacement = {
    marker: SeedMapMarker
    point: { x: number; y: number }
    scale: number
    selected: boolean
    pin: Rect
    label?: Rect & { text: string }
}

const FONT = '600 12px system-ui'
const PIN_WIDTH = 22
const PIN_HEIGHT = 30

function intersects(a: Rect, b: Rect, gap = 0): boolean {
    return (
        a.x < b.x + b.width + gap &&
        a.x + a.width + gap > b.x &&
        a.y < b.y + b.height + gap &&
        a.y + a.height + gap > b.y
    )
}

function contains(rect: Rect, x: number, y: number, padding = 0): boolean {
    return (
        x >= rect.x - padding &&
        x <= rect.x + rect.width + padding &&
        y >= rect.y - padding &&
        y <= rect.y + rect.height + padding
    )
}

function fitName(name: string, width: number, measure: (text: string) => number): string {
    if (measure(name) <= width) return name
    const characters = Array.from(name)
    let min = 0
    let max = characters.length
    while (min < max) {
        const length = Math.ceil((min + max) / 2)
        if (measure(characters.slice(0, length).join('') + '…') <= width) min = length
        else max = length - 1
    }
    return characters.slice(0, min).join('') + '…'
}

/** Screen-space geometry shared by drawing and selection, independent of map LoD. */
export function seedMapMarkerLayout(
    markers: SeedMapMarker[],
    project: (x: number, z: number) => { x: number; y: number },
    viewport: { width: number; height: number },
    scale: number,
    selectedId: string | undefined,
    measure: (text: string) => number,
): SeedMapMarkerPlacement[] {
    const visible: SeedMapMarkerPlacement[] = markers
        .map((marker) => {
            const point = project(marker.x, marker.z)
            return {
                marker,
                point,
                scale,
                selected: marker.id === selectedId,
                pin: {
                    x: point.x - (PIN_WIDTH * scale) / 2,
                    y: point.y - PIN_HEIGHT * scale,
                    width: PIN_WIDTH * scale,
                    height: PIN_HEIGHT * scale,
                },
            }
        })
        .filter(({ pin }) => intersects(pin, { x: 0, y: 0, ...viewport }))
        .sort((a, b) => Number(a.selected) - Number(b.selected))
    const occupied: Rect[] = []
    const maxWidth = Math.min(160, viewport.width - 24)
    if (maxWidth <= measure('…')) return visible
    for (const placement of [...visible].reverse()) {
        const text = fitName(placement.marker.name, maxWidth, measure)
        const width = measure(text) + 8
        const label = {
            text,
            x: Math.max(4, Math.min(viewport.width - width - 4, placement.point.x - width / 2)),
            y: placement.pin.y - 24,
            width,
            height: 18,
        }
        if (
            label.y < 4 ||
            visible.some(({ pin }) => intersects(label, pin, 3)) ||
            occupied.some((rect) => intersects(label, rect, 4))
        )
            continue
        placement.label = label
        occupied.push(label)
    }
    return visible
}

export function layoutSeedMapMarkers(
    context: CanvasRenderingContext2D,
    markers: SeedMapMarker[],
    project: (x: number, z: number) => { x: number; y: number },
    viewport: { width: number; height: number },
    scale: number,
    selectedId: string | undefined,
): SeedMapMarkerPlacement[] {
    context.save()
    context.font = FONT
    const result = seedMapMarkerLayout(
        markers,
        project,
        viewport,
        scale,
        selectedId,
        (text) => context.measureText(text).width,
    )
    context.restore()
    return result
}

export function drawSeedMapMarkers(
    context: CanvasRenderingContext2D,
    placements: SeedMapMarkerPlacement[],
) {
    context.save()
    for (const { marker, point, scale, selected } of placements) {
        context.save()
        context.translate(point.x, point.y)
        context.scale(scale, scale)
        context.beginPath()
        context.moveTo(0, 0)
        context.bezierCurveTo(-3, -5, -11, -15, -11, -19)
        context.bezierCurveTo(-11, -25, -6, -30, 0, -30)
        context.bezierCurveTo(6, -30, 11, -25, 11, -19)
        context.bezierCurveTo(11, -15, 3, -5, 0, 0)
        context.closePath()
        context.lineJoin = 'round'
        context.shadowColor = 'rgba(0, 0, 0, 0.5)'
        context.shadowBlur = 4
        context.shadowOffsetY = 2
        context.fillStyle = marker.color
        context.fill()
        context.shadowColor = 'transparent'
        if (selected) {
            context.strokeStyle = 'rgba(0, 0, 0, 0.75)'
            context.lineWidth = 5
            context.stroke()
        }
        context.strokeStyle = '#FFFFFF'
        context.lineWidth = selected ? 3 : 2
        context.stroke()
        context.restore()
    }
    context.font = FONT
    context.textAlign = 'center'
    context.textBaseline = 'middle'
    context.lineJoin = 'round'
    context.lineWidth = 3
    context.strokeStyle = 'rgba(0, 0, 0, 0.85)'
    context.fillStyle = '#FFFFFF'
    for (const { label } of placements) {
        if (!label) continue
        const x = label.x + label.width / 2
        const y = label.y + label.height / 2
        context.strokeText(label.text, x, y)
        context.fillText(label.text, x, y)
    }
    context.restore()
}

export function hitSeedMapMarker(placements: SeedMapMarkerPlacement[], x: number, y: number) {
    const frontToBack = [...placements].reverse()
    return (
        frontToBack.find(({ label }) => label && contains(label, x, y)) ??
        frontToBack.find(({ pin }) => contains(pin, x, y, 3))
    )?.marker
}
