export type ScreenshotGrid = {
    gridTop: number
    gridHeight: number
    isOpen: boolean
    count: number
}

/** Share the two-image lookahead across groups, including collapsed/empty groups. */
export function screenshotRenderWindows(
    grids: ScreenshotGrid[],
    viewportStart: number,
    viewportEnd: number,
    columns: number,
    rowHeight: number,
) {
    let remainingLookahead = 2
    return grids.map((grid) => {
        if (!grid.isOpen || !grid.count || grid.gridTop + grid.gridHeight < viewportStart) {
            return { start: 0, end: 0, top: 0 }
        }
        const rowCount = Math.ceil(grid.count / columns)
        const firstRow = Math.min(
            rowCount,
            Math.max(0, Math.floor((viewportStart - grid.gridTop) / rowHeight)),
        )
        const lastRow = Math.min(
            rowCount,
            Math.max(firstRow, Math.ceil((viewportEnd - grid.gridTop) / rowHeight)),
        )
        let end = Math.min(grid.count, lastRow * columns)
        if (grid.gridTop + grid.gridHeight >= viewportEnd) {
            const lookahead = Math.min(remainingLookahead, grid.count - end)
            end += lookahead
            remainingLookahead -= lookahead
        }
        return { start: firstRow * columns, end, top: firstRow * rowHeight }
    })
}
