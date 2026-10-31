import { defineMessages } from '@modrinth/ui'

export const strongholdMessages = defineMessages({
    copyTeleport: {
        id: 'app.lab.stronghold.copy-teleport',
        defaultMessage: 'Copy teleport command',
    },
    selected: { id: 'app.lab.stronghold.selected', defaultMessage: 'Selected chunk' },
    supporters: { id: 'app.lab.stronghold.supporters', defaultMessage: 'Supporting measurements' },
    searchRadius: {
        id: 'app.lab.stronghold.search-radius',
        defaultMessage: 'Search radius from world origin (blocks)',
    },
    plotGeneration: {
        id: 'app.lab.stronghold.plot.generation',
        defaultMessage: 'Generation range · not confirmed strongholds',
    },
    plotGenerationFit: {
        id: 'app.lab.stronghold.plot.generation-fit',
        defaultMessage: 'Show generation range',
    },
    plotGenerationChunk: {
        id: 'app.lab.stronghold.plot.generation-chunk',
        defaultMessage:
            'Chunk {x}, {z} · can generate a stronghold; not in the current candidate set',
    },
    plotOutside: {
        id: 'app.lab.stronghold.plot.outside',
        defaultMessage: 'Chunk {x}, {z} · outside the current candidate set',
    },
    plotTitle: { id: 'app.lab.stronghold.plot.title', defaultMessage: 'Observation map' },
    plotHelp: {
        id: 'app.lab.stronghold.plot.help',
        defaultMessage:
            'Drag to pan; scroll to zoom. Click a chunk to select it. Arrow keys move; +/\u2212 zoom; Home fits the view.',
    },
    plotZoomIn: { id: 'app.lab.stronghold.plot.zoom-in', defaultMessage: 'Zoom in' },
    plotZoomOut: { id: 'app.lab.stronghold.plot.zoom-out', defaultMessage: 'Zoom out' },
    plotFit: {
        id: 'app.lab.stronghold.plot.fit',
        defaultMessage: 'Fit observations and candidates',
    },
    plotHeat: {
        id: 'app.lab.stronghold.plot.heat',
        defaultMessage: 'Support score \u00b7 brighter means higher support',
    },
    plotBest: { id: 'app.lab.stronghold.plot.best', defaultMessage: 'Top candidate' },
    plotHover: {
        id: 'app.lab.stronghold.plot.hover',
        defaultMessage: 'Point to inspect; click a chunk to get its coordinates.',
    },
    plotChunk: {
        id: 'app.lab.stronghold.plot.chunk',
        defaultMessage: 'Chunk {x}, {z} \u00b7 support score {probability}',
    },
    plotAccessible: {
        id: 'app.lab.stronghold.plot.accessible',
        defaultMessage:
            'Overworld X/Z map with {count} observation rays and {candidates} predicted candidate chunks',
    },
    title: { id: 'app.lab.stronghold.title', defaultMessage: 'Stronghold locator' },
    description: {
        id: 'app.lab.stronghold.description',
        defaultMessage: 'Locate a Java Edition stronghold using Eye of Ender measurements.',
    },
    input: { id: 'app.lab.stronghold.input', defaultMessage: 'Add a measurement' },
    paste: { id: 'app.lab.stronghold.paste', defaultMessage: 'Paste F3+C' },
    manual: { id: 'app.lab.stronghold.manual', defaultMessage: 'Enter manually' },
    pasteHelp: {
        id: 'app.lab.stronghold.paste-help',
        defaultMessage:
            'In the Overworld, aim at a thrown Eye of Ender and press F3+C. Paste the copied command here; one command per line.',
    },
    placeholder: {
        id: 'app.lab.stronghold.placeholder',
        defaultMessage: 'Paste a complete /execute command',
    },
    add: { id: 'app.lab.stronghold.add', defaultMessage: 'Add measurement' },
    yaw: { id: 'app.lab.stronghold.yaw', defaultMessage: 'Yaw (°)' },
    version: { id: 'app.lab.stronghold.version', defaultMessage: 'Minecraft version' },
    pre119: { id: 'app.lab.stronghold.pre119', defaultMessage: 'Java 1.9–1.18.2' },
    post119: { id: 'app.lab.stronghold.post119', defaultMessage: 'Java 1.19 and later' },
    observations: {
        id: 'app.lab.stronghold.observations',
        defaultMessage: 'Measurements ({count})',
    },
    clear: { id: 'app.lab.stronghold.clear', defaultMessage: 'Clear all' },
    remove: { id: 'app.lab.stronghold.remove', defaultMessage: 'Remove measurement {index}' },
    noMeasurements: {
        id: 'app.lab.stronghold.no-measurements',
        defaultMessage: 'No measurements yet',
    },
    advanced: { id: 'app.lab.stronghold.advanced', defaultMessage: 'Measurement settings' },
    precision: { id: 'app.lab.stronghold.precision', defaultMessage: 'Sector half-angle (\u00b0)' },
    precisionHelp: {
        id: 'app.lab.stronghold.precision-help',
        defaultMessage: 'Use a smaller value only for carefully calibrated measurements.',
    },
    closest: {
        id: 'app.lab.stronghold.closest',
        defaultMessage: 'Account for the nearest stronghold',
    },
    results: { id: 'app.lab.stronghold.results', defaultMessage: 'Candidate chunks' },
    waiting: { id: 'app.lab.stronghold.waiting', defaultMessage: 'Start with an Eye of Ender' },
    waitingHelp: {
        id: 'app.lab.stronghold.waiting-help',
        defaultMessage: 'Add measurements to see supported chunks on the map.',
    },
    oneEye: {
        id: 'app.lab.stronghold.one-eye',
        defaultMessage:
            'Only one measurement. Move to another position and throw another eye to narrow the search.',
    },
    calculating: {
        id: 'app.lab.stronghold.calculating',
        defaultMessage: 'Calculating candidates…',
    },
    noResult: {
        id: 'app.lab.stronghold.no-result',
        defaultMessage: 'No supported chunks in this range',
    },
    noResultHelp: {
        id: 'app.lab.stronghold.no-result-help',
        defaultMessage:
            'Check the measurement directions, widen the sector, or increase the search radius.',
    },
    computeError: {
        id: 'app.lab.stronghold.compute-error',
        defaultMessage: 'Calculation failed. Change a measurement to try again.',
    },
    probability: { id: 'app.lab.stronghold.probability', defaultMessage: 'Support score' },
    chunk: { id: 'app.lab.stronghold.chunk', defaultMessage: 'Chunk' },
    overworld: { id: 'app.lab.stronghold.overworld', defaultMessage: 'Overworld' },
    nether: { id: 'app.lab.stronghold.nether', defaultMessage: 'Nether travel reference' },
    distance: {
        id: 'app.lab.stronghold.distance',
        defaultMessage: '{distance, number} blocks from your last measurement',
    },
    locationHelp: {
        id: 'app.lab.stronghold.location-help',
        defaultMessage:
            'This points to the stronghold starting-staircase chunk. Use another eye when you arrive to confirm the location.',
    },
    candidates: { id: 'app.lab.stronghold.candidates', defaultMessage: 'Top candidates' },
    copy: { id: 'app.lab.stronghold.copy', defaultMessage: 'Copy X/Z' },
    copied: { id: 'app.lab.stronghold.copied', defaultMessage: 'Copied' },
    copyError: {
        id: 'app.lab.stronghold.copy-error',
        defaultMessage: 'Could not copy. Select the coordinates and copy them manually.',
    },
    format: {
        id: 'app.lab.stronghold.error.format',
        defaultMessage: 'Paste a complete F3+C command, or enter X, Z and yaw manually.',
    },
    dimension: {
        id: 'app.lab.stronghold.error.dimension',
        defaultMessage: 'Take this measurement in the Overworld.',
    },
    range: {
        id: 'app.lab.stronghold.error.range',
        defaultMessage: 'Enter finite numbers. X and Z must be within the world border.',
    },
    duplicate: {
        id: 'app.lab.stronghold.error.duplicate',
        defaultMessage: 'This measurement has already been added.',
    },
    limit: {
        id: 'app.lab.stronghold.error.limit',
        defaultMessage: 'You can add up to 100 measurements. Remove some before adding more.',
    },
    sigmaError: {
        id: 'app.lab.stronghold.error.sigma',
        defaultMessage:
            'Enter a half-angle greater than 0 and at most 90\u00b0, and a radius from 1000 to 26000 blocks.',
    },
})
