/**
 * Measures actual overflow. Tooltip owns observation through `onlyWhenTruncated`.
 */
export function isElementTruncated(element: HTMLElement): boolean {
    return (
        element.scrollWidth > element.clientWidth || element.scrollHeight > element.clientHeight + 1
    )
}
