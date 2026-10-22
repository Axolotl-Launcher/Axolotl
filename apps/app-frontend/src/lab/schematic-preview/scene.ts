import * as THREE from 'three'
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js'
import { PointerLockControls } from 'three/examples/jsm/controls/PointerLockControls.js'

import type { SchematicMeshData } from './mesh-worker'

export type SchematicSceneRegion = {
    id: string
    min: [number, number, number]
    max: [number, number, number]
}

export type SchematicSceneSelection = {
    regionId: string
    position: [number, number, number]
}

type SceneOptions = {
    canvas: HTMLCanvasElement
    onSelect: (selection?: SchematicSceneSelection) => void
    onFocus: (selection: SchematicSceneSelection) => void
    onContextLost: () => void
    onContextRestored: () => void
    onViewModeChange: (mode: ViewMode) => void
    onWalkLockChange: (locked: boolean) => void
    onWalkSpeedChange: (speed: number) => void
    onNativeWalkLock?: () => Promise<boolean>
    onNativeWalkUnlock?: () => Promise<void>
    requireWalkLock?: boolean
    doubleSided?: boolean
    releaseContextOnDispose?: boolean
}

type Projection = 'perspective' | 'orthographic'
export type ViewMode = 'orbit' | 'walk'
export type SchematicSelectionBounds = {
    min: [number, number, number]
    max: [number, number, number]
}

const ORBIT_FOV = 50
const WALK_FOV = 90
const WALK_SPEED_MINIMUM = 4
const WALK_SPEED_MAXIMUM = 60
const WALK_SPEED_MULTIPLIER = 0.85
const NATIVE_WALK_MOUSE_DELTA_MAXIMUM = 128

export function filterNativeWalkMouseDelta(delta: number) {
    return Number.isFinite(delta) && Math.abs(delta) <= NATIVE_WALK_MOUSE_DELTA_MAXIMUM ? delta : 0
}

export class SchematicPreviewScene {
    private readonly canvas: HTMLCanvasElement
    private readonly renderer: THREE.WebGLRenderer
    private readonly scene = new THREE.Scene()
    private readonly perspectiveCamera = new THREE.PerspectiveCamera(50, 1, 0.05, 10000)
    private readonly orthographicCamera = new THREE.OrthographicCamera(
        -32,
        32,
        32,
        -32,
        -10000,
        10000,
    )
    private readonly controls: OrbitControls
    private readonly walkControls: PointerLockControls
    private readonly raycaster = new THREE.Raycaster()
    private readonly pointer = new THREE.Vector2()
    private readonly regions = new Map<string, THREE.Group>()
    private readonly regionBounds = new THREE.Group()
    private readonly measurement = new THREE.Group()
    private readonly chunks = new Map<string, THREE.Group>()
    private readonly resizeObserver: ResizeObserver
    private readonly onSelect: SceneOptions['onSelect']
    private readonly onFocus: SceneOptions['onFocus']
    private activeCamera: THREE.Camera = this.perspectiveCamera
    private projection: Projection = 'perspective'
    private opaqueMaterial = this.createMaterial(false)
    private translucentMaterial = this.createMaterial(true)
    private grid?: THREE.Group
    private selection = new THREE.Group()
    private selectionBounds?: SchematicSelectionBounds
    private selectionEntries: SchematicSceneSelection[] = []
    private measurementStart?: SchematicSceneSelection
    private measurementEnd?: SchematicSceneSelection
    private renderFrame?: number
    private controlFrame?: number
    private walkFrame?: number
    private bounds?: THREE.Box3
    private layerRange?: [number, number]
    private pointerStart?: THREE.Vector2
    private disposed = false
    private gridVisible = true
    private boundsVisible = true
    private translucentVisible = true
    private explosion = 0
    private viewMode: ViewMode = 'orbit'
    private walkPreviousTime = 0
    private walkSpeed = 8
    private walkOrbitTarget?: THREE.Vector3
    private readonly pressedKeys = new Set<string>()
    private readonly onViewModeChange: SceneOptions['onViewModeChange']
    private readonly onWalkLockChange: SceneOptions['onWalkLockChange']
    private readonly onWalkSpeedChange: SceneOptions['onWalkSpeedChange']
    private readonly onNativeWalkLock?: SceneOptions['onNativeWalkLock']
    private readonly onNativeWalkUnlock?: SceneOptions['onNativeWalkUnlock']
    private readonly walkDirection = new THREE.Vector3()
    private readonly walkForward = new THREE.Vector3()
    private readonly walkRight = new THREE.Vector3()
    private readonly walkEuler = new THREE.Euler(0, 0, 0, 'YXZ')
    private nativeWalkLockPending = false
    private nativeWalkLocked = false
    private readonly requireWalkLock: boolean
    private readonly releaseContextOnDispose: boolean
    private readonly onContextLost: SceneOptions['onContextLost']
    private readonly onContextRestored: SceneOptions['onContextRestored']

    constructor(options: SceneOptions) {
        this.canvas = options.canvas
        this.onSelect = options.onSelect
        this.onFocus = options.onFocus
        this.onViewModeChange = options.onViewModeChange
        this.onWalkLockChange = options.onWalkLockChange
        this.onWalkSpeedChange = options.onWalkSpeedChange
        this.onNativeWalkLock = options.onNativeWalkLock
        this.onNativeWalkUnlock = options.onNativeWalkUnlock
        this.requireWalkLock = options.requireWalkLock ?? false
        this.releaseContextOnDispose = options.releaseContextOnDispose ?? false
        this.onContextLost = options.onContextLost
        this.onContextRestored = options.onContextRestored
        if (options.doubleSided) {
            this.opaqueMaterial.side = THREE.DoubleSide
            this.translucentMaterial.side = THREE.DoubleSide
        }
        this.renderer = new THREE.WebGLRenderer({
            canvas: options.canvas,
            antialias: true,
            alpha: false,
            preserveDrawingBuffer: true,
        })
        this.renderer.setClearColor(new THREE.Color('#0a0a0a'), 1)
        this.renderer.outputColorSpace = THREE.SRGBColorSpace
        this.renderer.toneMapping = THREE.ACESFilmicToneMapping
        this.renderer.toneMappingExposure = 1.5
        this.renderer.localClippingEnabled = true
        this.renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2))
        this.scene.add(this.regionBounds)
        this.scene.add(this.measurement)
        this.scene.add(this.selection)
        this.scene.add(new THREE.AmbientLight('#ffffff', 0.8))
        this.scene.add(new THREE.HemisphereLight('#87ceeb', '#444444', 0.6))
        const keyLight = new THREE.DirectionalLight('#fffbf0', 4)
        keyLight.position.set(-100, 151, 50)
        this.scene.add(keyLight)
        const fillLight = new THREE.DirectionalLight('#ffffff', 0.3)
        fillLight.position.set(-30, 40, -30)
        this.scene.add(fillLight)

        this.perspectiveCamera.position.set(24, 24, 24)
        this.controls = new OrbitControls(this.activeCamera, this.canvas)
        this.controls.enableDamping = true
        this.controls.dampingFactor = 0.1
        this.controls.minDistance = 3
        this.controls.screenSpacePanning = true
        this.controls.zoomToCursor = true
        this.controls.addEventListener('change', () => this.requestRender())
        this.controls.addEventListener('start', () => this.startControlLoop())
        this.walkControls = new PointerLockControls(this.perspectiveCamera, this.canvas)
        this.walkControls.enabled = false
        this.walkControls.addEventListener('lock', this.handleWalkLock)
        this.walkControls.addEventListener('unlock', this.handleWalkUnlock)
        this.resizeObserver = new ResizeObserver(() => this.resize())
        this.resizeObserver.observe(this.canvas.parentElement ?? this.canvas)
        this.canvas.addEventListener('pointerdown', this.handlePointerDown)
        this.canvas.addEventListener('pointerup', this.handlePointerUp)
        this.canvas.ownerDocument.addEventListener('mousemove', this.handleWalkMouseMove)
        this.canvas.ownerDocument.addEventListener('wheel', this.handleWalkWheel, {
            passive: false,
            capture: true,
        })
        this.canvas.ownerDocument.addEventListener(
            'pointerlockerror',
            this.handleWalkPointerLockError,
        )
        window.addEventListener('keydown', this.handleWalkKeyDown)
        window.addEventListener('keyup', this.handleWalkKeyUp)
        window.addEventListener('blur', this.handleWalkBlur)
        this.canvas.addEventListener('webglcontextlost', this.handleContextLost)
        this.canvas.addEventListener('webglcontextrestored', this.handleContextRestored)
        this.resize()
        this.onWalkSpeedChange(this.walkSpeed)
    }

    setRegions(regions: SchematicSceneRegion[]) {
        this.setSelection()
        this.setMeasurement()
        for (const group of this.regions.values()) {
            this.scene.remove(group)
            this.disposeObject(group)
        }
        this.regions.clear()
        this.chunks.clear()
        for (const helper of this.regionBounds.children) this.disposeHelper(helper)
        this.regionBounds.clear()
        const bounds = new THREE.Box3()
        for (const region of regions) {
            const group = new THREE.Group()
            group.name = region.id
            this.regions.set(region.id, group)
            this.scene.add(group)
            const box = new THREE.Box3(
                new THREE.Vector3(...region.min),
                new THREE.Vector3(region.max[0] + 1, region.max[1] + 1, region.max[2] + 1),
            )
            bounds.union(box)
            const helper = new THREE.Box3Helper(box, new THREE.Color('#58736a'))
            const material = helper.material as THREE.LineBasicMaterial
            material.transparent = true
            material.opacity = 0.52
            material.depthWrite = false
            helper.userData.regionId = region.id
            this.regionBounds.add(helper)
        }
        this.bounds = bounds.isEmpty() ? undefined : bounds
        this.rebuildGrid()
        this.requestRender()
    }

    setTexture(texture?: THREE.CanvasTexture) {
        const oldOpaque = this.opaqueMaterial
        const oldTranslucent = this.translucentMaterial
        this.opaqueMaterial = this.createMaterial(false, texture)
        this.translucentMaterial = this.createMaterial(true, texture)
        this.opaqueMaterial.side = oldOpaque.side
        this.translucentMaterial.side = oldTranslucent.side
        for (const chunk of this.chunks.values()) {
            for (const child of chunk.children) {
                if (!(child instanceof THREE.Mesh)) continue
                child.material = child.userData.translucent
                    ? this.translucentMaterial
                    : this.opaqueMaterial
            }
        }
        oldOpaque.dispose()
        oldTranslucent.dispose()
        this.applyClippingPlanes()
        this.requestRender()
    }

    clearChunks() {
        for (const chunk of this.chunks.values()) {
            chunk.parent?.remove(chunk)
            this.disposeObject(chunk)
        }
        this.chunks.clear()
        if (this.selectionEntries.length > 0) this.rebuildSelection()
        else this.requestRender()
    }

    setChunk(
        regionId: string,
        chunkPosition: [number, number, number],
        opaque: SchematicMeshData,
        translucent: SchematicMeshData,
    ) {
        const key = `${regionId}:${chunkPosition.join(':')}`
        const previous = this.chunks.get(key)
        if (previous) {
            previous.parent?.remove(previous)
            this.disposeObject(previous)
        }
        const group = new THREE.Group()
        group.userData.chunkPosition = chunkPosition
        const opaqueMesh = this.createMesh(opaque, false, regionId)
        if (opaqueMesh) group.add(opaqueMesh)
        const translucentMesh = this.createMesh(translucent, true, regionId)
        if (translucentMesh) {
            translucentMesh.visible = this.translucentVisible
            group.add(translucentMesh)
        }
        this.regions.get(regionId)?.add(group)
        this.chunks.set(key, group)
        const selectionChanged = this.selectionEntries.some(
            (selection) =>
                selection.regionId === regionId &&
                Math.floor(selection.position[0] / 16) === chunkPosition[0] &&
                Math.floor(selection.position[1] / 16) === chunkPosition[1] &&
                Math.floor(selection.position[2] / 16) === chunkPosition[2],
        )
        if (selectionChanged) this.rebuildSelection()
        else this.requestRender()
    }

    setRegionVisible(regionId: string, visible: boolean) {
        const group = this.regions.get(regionId)
        if (group) group.visible = visible
        for (const child of this.regionBounds.children) {
            if (child.userData.regionId === regionId) child.visible = visible && this.boundsVisible
        }
        this.requestRender()
    }

    removeChunk(regionId: string, position: [number, number, number]) {
        const key = `${regionId}:${position.join(':')}`
        const chunk = this.chunks.get(key)
        if (!chunk) return
        chunk.parent?.remove(chunk)
        this.disposeObject(chunk)
        this.chunks.delete(key)
        this.requestRender()
    }

    setChunkOffset(
        regionId: string,
        position: [number, number, number],
        offset: [number, number, number],
    ) {
        this.chunks.get(`${regionId}:${position.join(':')}`)?.position.set(...offset)
        this.requestRender()
    }

    crosshairBlock(): [number, number, number] | undefined {
        this.scene.updateMatrixWorld(true)
        this.raycaster.setFromCamera(new THREE.Vector2(0, 0), this.activeCamera)
        const candidates: THREE.Object3D[] = []
        for (const group of this.regions.values())
            if (group.visible) candidates.push(...group.children)
        const hit = this.raycaster
            .intersectObjects(candidates, true)
            .find(
                (hit) =>
                    hit.object instanceof THREE.Mesh &&
                    hit.object.visible &&
                    hit.faceIndex !== undefined &&
                    hit.distance <= 256,
            )
        if (!hit || !(hit.object instanceof THREE.Mesh)) return
        const attribute = hit.object.geometry.getAttribute('blockPosition')
        if (!attribute) return
        const point = new THREE.Vector3()
            .fromBufferAttribute(attribute, (hit.faceIndex ?? 0) * 3)
            .applyMatrix4(hit.object.matrixWorld)
        return [Math.round(point.x), Math.round(point.y), Math.round(point.z)]
    }

    setLayerRange(range?: [number, number]) {
        this.layerRange = range
        this.applyClippingPlanes()
        this.requestRender()
    }

    setGridVisible(visible: boolean) {
        this.gridVisible = visible
        if (this.grid) this.grid.visible = visible
        this.requestRender()
    }

    setBoundsVisible(visible: boolean) {
        this.boundsVisible = visible
        this.regionBounds.visible = visible && this.explosion === 0
        this.requestRender()
    }

    setTranslucentVisible(visible: boolean) {
        this.translucentVisible = visible
        for (const chunk of this.chunks.values()) {
            for (const child of chunk.children) {
                if (child.userData.translucent) child.visible = visible
            }
        }
        this.requestRender()
    }

    setExplosion(value: number) {
        this.explosion = Math.max(0, Math.min(1, value))
        this.regionBounds.visible = this.boundsVisible && this.explosion === 0
        for (const chunk of this.chunks.values()) {
            for (const child of chunk.children) {
                if (child instanceof THREE.Mesh) this.applyExplosion(child)
            }
        }
        this.setSelectionBounds(this.selectionBounds)
        this.setMeasurement(this.measurementStart, this.measurementEnd)
    }

    setProjection(projection: Projection) {
        if (projection === this.projection) return
        const previous = this.activeCamera
        this.projection = projection
        this.activeCamera =
            projection === 'perspective' ? this.perspectiveCamera : this.orthographicCamera
        this.activeCamera.position.copy(previous.position)
        this.activeCamera.quaternion.copy(previous.quaternion)
        this.controls.object = this.activeCamera
        this.resize()
        this.controls.update()
        this.requestRender()
    }

    setViewMode(mode: ViewMode) {
        if (mode === this.viewMode) return
        this.viewMode = mode
        this.controls.enabled = mode === 'orbit'
        this.walkControls.enabled = mode === 'walk'
        if (mode === 'walk') {
            if (this.projection !== 'perspective') this.setProjection('perspective')
            this.walkOrbitTarget = this.controls.target.clone()
            this.walkPreviousTime = performance.now()
            this.startWalkLoop()
            queueMicrotask(() => {
                if (!this.disposed && this.viewMode === 'walk') {
                    this.canvas.ownerDocument.addEventListener('click', this.handleWalkClick)
                }
            })
        } else {
            this.pressedKeys.clear()
            this.canvas.ownerDocument.removeEventListener('click', this.handleWalkClick)
            if (this.walkControls.isLocked) this.walkControls.unlock()
            void this.releaseNativeWalkLock()
            if (this.walkOrbitTarget) this.controls.target.copy(this.walkOrbitTarget)
            this.walkOrbitTarget = undefined
            this.controls.update()
            this.startWalkLoop()
        }
        this.onViewModeChange(mode)
        this.requestRender()
    }

    getCameraPosition(): [number, number, number] {
        return this.activeCamera.position.toArray()
    }

    setCameraPosition(position: [number, number, number]) {
        const direction = this.activeCamera.getWorldDirection(new THREE.Vector3())
        this.activeCamera.position.set(...position)
        this.controls.target.copy(this.activeCamera.position).addScaledVector(direction, 8)
        this.controls.update()
        this.requestRender()
    }

    fitView(targetBounds = this.explodedBounds(this.bounds)) {
        if (!targetBounds || targetBounds.isEmpty()) return
        const center = targetBounds.getCenter(new THREE.Vector3())
        const size = targetBounds.getSize(new THREE.Vector3())
        const maximum = Math.max(size.x, size.y, size.z, 4)
        this.controls.target.copy(center)
        this.activeCamera.position
            .copy(center)
            .add(new THREE.Vector3(maximum, maximum * 0.8, maximum))
        if (this.activeCamera instanceof THREE.PerspectiveCamera) {
            this.activeCamera.near = Math.max(0.05, maximum / 1000)
            this.activeCamera.far = Math.max(1000, maximum * 20)
            this.activeCamera.updateProjectionMatrix()
        } else {
            this.activeCamera.zoom = Math.min(8, 42 / maximum)
            this.activeCamera.updateProjectionMatrix()
        }
        this.controls.update()
        this.requestRender()
    }

    focusRegion(region: SchematicSceneRegion) {
        this.fitView(
            this.explodedBounds(
                new THREE.Box3(
                    new THREE.Vector3(...region.min),
                    new THREE.Vector3(region.max[0] + 1, region.max[1] + 1, region.max[2] + 1),
                ),
            ),
        )
    }

    setSelection(selection?: SchematicSceneSelection) {
        this.setSelectionBlocks(
            selection ? [selection] : [],
            selection ? { min: selection.position, max: selection.position } : undefined,
        )
    }

    setSelectionBounds(bounds?: SchematicSelectionBounds) {
        this.selectionBounds = bounds
        this.rebuildSelection()
    }

    setSelectionBlocks(
        selections: readonly SchematicSceneSelection[],
        bounds?: SchematicSelectionBounds,
    ) {
        this.selectionEntries = selections.slice(0, 4000)
        this.selectionBounds = bounds
        this.rebuildSelection()
    }

    private rebuildSelection() {
        for (const child of this.selection.children) this.disposeHelper(child)
        this.selection.clear()
        const bounds = this.selectionBounds
        if (!bounds) {
            this.requestRender()
            return
        }

        const color = this.accentColor()
        if (this.selectionEntries.length > 0) {
            const surfaceGeometry = this.selectionSurfaceGeometry(this.selectionEntries)
            const surface = new THREE.Mesh(
                surfaceGeometry,
                new THREE.MeshBasicMaterial({
                    color,
                    transparent: true,
                    opacity: 0.24,
                    depthTest: true,
                    depthWrite: false,
                    polygonOffset: true,
                    polygonOffsetFactor: -8,
                    polygonOffsetUnits: -8,
                    side: THREE.DoubleSide,
                    toneMapped: false,
                }),
            )
            surface.renderOrder = 30
            surface.raycast = () => {}
            this.selection.add(surface)

            const outline = new THREE.LineSegments(
                new THREE.EdgesGeometry(surfaceGeometry, 1),
                new THREE.LineBasicMaterial({
                    color,
                    transparent: true,
                    opacity: 1,
                    depthTest: false,
                    depthWrite: false,
                }),
            )
            outline.renderOrder = 31
            outline.raycast = () => {}
            this.selection.add(outline)
        }

        if (this.selectionEntries.length !== 1) {
            const floor = this.bounds?.min.y ?? bounds.min[1]
            const minY = bounds.min[1] + (bounds.min[1] - floor) * this.explosion
            const maxY = bounds.max[1] + 1 + (bounds.max[1] - floor) * this.explosion
            const helper = new THREE.Box3Helper(
                new THREE.Box3(
                    new THREE.Vector3(bounds.min[0] - 0.025, minY - 0.025, bounds.min[2] - 0.025),
                    new THREE.Vector3(bounds.max[0] + 1.025, maxY + 0.025, bounds.max[2] + 1.025),
                ),
                color,
            )
            const material = helper.material as THREE.LineBasicMaterial
            material.transparent = true
            material.opacity = 0.65
            material.depthTest = false
            material.depthWrite = false
            helper.renderOrder = 29
            this.selection.add(helper)
        }
        this.requestRender()
    }

    private selectionSurfaceGeometry(selections: readonly SchematicSceneSelection[]) {
        const vertices: number[] = []
        for (const selection of selections) {
            if (!this.appendModelSurface(vertices, selection)) {
                this.appendCubeSurface(vertices, selection.position)
            }
        }
        return new THREE.BufferGeometry().setAttribute(
            'position',
            new THREE.BufferAttribute(new Float32Array(vertices), 3),
        )
    }

    private appendModelSurface(vertices: number[], selection: SchematicSceneSelection) {
        let found = false
        const chunkPosition = selection.position.map((value) => Math.floor(value / 16))
        const chunk = this.chunks.get(`${selection.regionId}:${chunkPosition.join(':')}`)
        if (!chunk) return false
        for (const child of chunk.children) {
            if (!(child instanceof THREE.Mesh)) continue
            const positions = child.geometry.getAttribute('position')
            const blocks = child.geometry.getAttribute('blockPosition')
            for (let quad = 0; quad + 5 < positions.count; quad += 6) {
                if (
                    Math.round(blocks.getX(quad)) !== selection.position[0] ||
                    Math.round(blocks.getY(quad)) !== selection.position[1] ||
                    Math.round(blocks.getZ(quad)) !== selection.position[2]
                ) {
                    continue
                }
                found = true
                for (let vertex = 0; vertex < 6; vertex += 1) {
                    vertices.push(
                        positions.getX(quad + vertex),
                        positions.getY(quad + vertex),
                        positions.getZ(quad + vertex),
                    )
                }
            }
        }
        return found
    }

    private appendCubeSurface(vertices: number[], position: [number, number, number]) {
        const center = this.explodedBlockCenter(position)
        const radius = 0.5025
        const corners = [
            [-radius, -radius, -radius],
            [radius, -radius, -radius],
            [radius, -radius, radius],
            [-radius, -radius, radius],
            [-radius, radius, -radius],
            [radius, radius, -radius],
            [radius, radius, radius],
            [-radius, radius, radius],
        ] as const
        for (const index of [
            0, 1, 2, 0, 2, 3, 4, 7, 6, 4, 6, 5, 0, 4, 5, 0, 5, 1, 1, 5, 6, 1, 6, 2, 2, 6, 7, 2, 7,
            3, 3, 7, 4, 3, 4, 0,
        ]) {
            vertices.push(
                center.x + corners[index][0],
                center.y + corners[index][1],
                center.z + corners[index][2],
            )
        }
    }

    setMeasurement(start?: SchematicSceneSelection, end?: SchematicSceneSelection) {
        this.measurementStart = start
        this.measurementEnd = end
        for (const child of this.measurement.children) this.disposeHelper(child)
        this.measurement.clear()
        if (!start) {
            this.requestRender()
            return
        }

        const color = this.accentColor()
        this.measurement.add(new THREE.Box3Helper(this.explodedBlockBounds(start.position), color))
        if (end) {
            this.measurement.add(
                new THREE.Box3Helper(this.explodedBlockBounds(end.position), color),
            )
            const line = new THREE.Line(
                new THREE.BufferGeometry().setFromPoints([
                    this.explodedBlockCenter(start.position),
                    this.explodedBlockCenter(end.position),
                ]),
                new THREE.LineBasicMaterial({ color, depthTest: false }),
            )
            line.renderOrder = 20
            this.measurement.add(line)
        }
        this.requestRender()
    }

    focusSelection(selection: SchematicSceneSelection) {
        const floor = this.bounds?.min.y ?? selection.position[1]
        const center = new THREE.Vector3(
            selection.position[0] + 0.5,
            selection.position[1] + 0.5 + (selection.position[1] - floor) * this.explosion,
            selection.position[2] + 0.5,
        )
        const delta = this.activeCamera.position.clone().sub(this.controls.target)
        this.controls.target.copy(center)
        this.activeCamera.position
            .copy(center)
            .add(delta.setLength(Math.max(4, delta.length() * 0.35)))
        this.controls.update()
        this.requestRender()
    }

    toPngDataUrl() {
        this.render()
        return this.canvas.toDataURL('image/png')
    }

    dispose() {
        if (this.disposed) return
        this.disposed = true
        if (this.renderFrame !== undefined) cancelAnimationFrame(this.renderFrame)
        if (this.controlFrame !== undefined) cancelAnimationFrame(this.controlFrame)
        this.resizeObserver.disconnect()
        this.canvas.removeEventListener('pointerdown', this.handlePointerDown)
        this.canvas.removeEventListener('pointerup', this.handlePointerUp)
        this.canvas.removeEventListener('webglcontextlost', this.handleContextLost)
        this.canvas.removeEventListener('webglcontextrestored', this.handleContextRestored)
        this.canvas.ownerDocument.removeEventListener('click', this.handleWalkClick)
        this.canvas.ownerDocument.removeEventListener('mousemove', this.handleWalkMouseMove)
        this.canvas.ownerDocument.removeEventListener(
            'pointerlockerror',
            this.handleWalkPointerLockError,
        )
        this.canvas.ownerDocument.removeEventListener('wheel', this.handleWalkWheel, true)
        window.removeEventListener('keydown', this.handleWalkKeyDown)
        window.removeEventListener('keyup', this.handleWalkKeyUp)
        window.removeEventListener('blur', this.handleWalkBlur)
        this.controls.dispose()
        this.walkControls.removeEventListener('lock', this.handleWalkLock)
        this.walkControls.removeEventListener('unlock', this.handleWalkUnlock)
        if (this.walkControls.isLocked) this.walkControls.unlock()
        void this.releaseNativeWalkLock()
        this.walkControls.dispose()
        if (this.walkFrame !== undefined) cancelAnimationFrame(this.walkFrame)
        for (const chunk of this.chunks.values()) this.disposeObject(chunk)
        this.chunks.clear()
        this.regions.clear()
        this.pressedKeys.clear()
        if (this.grid) {
            this.scene.remove(this.grid)
            for (const helper of this.grid.children) this.disposeHelper(helper)
            this.grid.clear()
            this.grid = undefined
        }
        for (const child of this.selection.children) this.disposeHelper(child)
        this.selection.clear()
        for (const helper of this.regionBounds.children) this.disposeHelper(helper)
        this.regionBounds.clear()
        for (const helper of this.measurement.children) this.disposeHelper(helper)
        this.measurement.clear()
        this.opaqueMaterial.dispose()
        this.translucentMaterial.dispose()
        this.scene.clear()
        this.renderer.dispose()
        if (this.releaseContextOnDispose) this.renderer.forceContextLoss()
    }

    private createMaterial(translucent: boolean, texture?: THREE.CanvasTexture) {
        if (texture) {
            texture.flipY = false
            texture.magFilter = THREE.NearestFilter
            texture.minFilter = THREE.NearestFilter
            texture.generateMipmaps = false
            texture.needsUpdate = true
        }
        return new THREE.MeshLambertMaterial({
            map: texture,
            vertexColors: true,
            alphaTest: translucent ? 0 : 0.08,
            transparent: translucent,
            opacity: translucent ? 0.68 : 1,
            depthWrite: !translucent,
        })
    }

    private accentColor() {
        const value = getComputedStyle(document.documentElement)
            .getPropertyValue('--color-brand')
            .trim()
        return new THREE.Color(value || '#86d562')
    }

    private createMesh(data: SchematicMeshData, translucent: boolean, regionId: string) {
        if (data.positions.length === 0) return undefined
        const geometry = new THREE.BufferGeometry()
        geometry.setAttribute('position', new THREE.BufferAttribute(data.positions, 3))
        geometry.setAttribute('normal', new THREE.BufferAttribute(data.normals, 3))
        geometry.setAttribute('uv', new THREE.BufferAttribute(data.uvs, 2))
        geometry.setAttribute('color', new THREE.BufferAttribute(data.colors, 3))
        geometry.setAttribute('blockPosition', new THREE.BufferAttribute(data.blockPositions, 3))
        geometry.computeBoundingBox()
        geometry.computeBoundingSphere()
        const mesh = new THREE.Mesh(
            geometry,
            translucent ? this.translucentMaterial : this.opaqueMaterial,
        )
        mesh.userData.regionId = regionId
        mesh.userData.translucent = translucent
        mesh.userData.basePositions = data.positions.slice()
        this.applyExplosion(mesh)
        return mesh
    }

    private applyExplosion(mesh: THREE.Mesh) {
        const position = mesh.geometry.getAttribute('position') as THREE.BufferAttribute
        const blockPosition = mesh.geometry.getAttribute('blockPosition') as THREE.BufferAttribute
        const basePositions = mesh.userData.basePositions as Float32Array | undefined
        if (!basePositions) return
        const floor = this.bounds?.min.y ?? 0
        for (let index = 0; index < position.count; index += 1) {
            position.setY(
                index,
                basePositions[index * 3 + 1] + (blockPosition.getY(index) - floor) * this.explosion,
            )
        }
        position.needsUpdate = true
        mesh.geometry.computeBoundingBox()
        mesh.geometry.computeBoundingSphere()
    }

    private explodedBounds(bounds?: THREE.Box3) {
        if (!bounds || this.explosion === 0) return bounds
        const exploded = bounds.clone()
        const floor = this.bounds?.min.y ?? bounds.min.y
        exploded.min.y += (bounds.min.y - floor) * this.explosion
        exploded.max.y += (bounds.max.y - 1 - floor) * this.explosion
        return exploded
    }

    private explodedBlockCenter(position: [number, number, number]) {
        const floor = this.bounds?.min.y ?? position[1]
        return new THREE.Vector3(
            position[0] + 0.5,
            position[1] + 0.5 + (position[1] - floor) * this.explosion,
            position[2] + 0.5,
        )
    }

    private explodedBlockBounds(position: [number, number, number]) {
        const center = this.explodedBlockCenter(position)
        return new THREE.Box3(center.clone().addScalar(-0.5), center.clone().addScalar(0.5))
    }

    private applyClippingPlanes() {
        const clippingPlanes = this.layerRange
            ? [
                  new THREE.Plane(new THREE.Vector3(0, 1, 0), -this.layerRange[0]),
                  new THREE.Plane(new THREE.Vector3(0, -1, 0), this.layerRange[1] + 1),
              ]
            : []
        this.opaqueMaterial.clippingPlanes = clippingPlanes
        this.translucentMaterial.clippingPlanes = clippingPlanes
        this.opaqueMaterial.needsUpdate = true
        this.translucentMaterial.needsUpdate = true
    }

    private rebuildGrid() {
        if (this.grid) {
            this.scene.remove(this.grid)
            for (const helper of this.grid.children) this.disposeHelper(helper)
            this.grid.clear()
        }
        if (!this.bounds) return
        const size = this.bounds.getSize(new THREE.Vector3())
        const center = this.bounds.getCenter(new THREE.Vector3())
        const gridSize = Math.max(200, Math.ceil((Math.max(size.x, size.z) * 1.5) / 24) * 24)
        const gridCenterX = Math.round(center.x / 12) * 12
        const gridCenterZ = Math.round(center.z / 12) * 12
        this.grid = new THREE.Group()
        const cells = new THREE.GridHelper(gridSize, Math.min(gridSize, 400), '#404040', '#404040')
        const sections = new THREE.GridHelper(
            gridSize,
            Math.max(1, Math.round(gridSize / 12)),
            '#6b6b6b',
            '#6b6b6b',
        )
        for (const [helper, opacity] of [
            [cells, 0.72],
            [sections, 0.9],
        ] as const) {
            helper.position.y = helper === cells ? 0 : 0.004
            const materials = Array.isArray(helper.material) ? helper.material : [helper.material]
            for (const material of materials) {
                material.transparent = true
                material.opacity = opacity
                material.depthWrite = false
            }
            this.grid.add(helper)
        }
        this.grid.position.set(gridCenterX, this.bounds.min.y - 0.02, gridCenterZ)
        this.grid.visible = this.gridVisible
        this.scene.add(this.grid)
    }

    private resize() {
        const parent = this.canvas.parentElement
        const width = Math.max(1, parent?.clientWidth ?? this.canvas.clientWidth)
        const height = Math.max(1, parent?.clientHeight ?? this.canvas.clientHeight)
        this.renderer.setSize(width, height, false)
        this.perspectiveCamera.aspect = width / height
        this.perspectiveCamera.updateProjectionMatrix()
        const span = 64
        this.orthographicCamera.left = (-span * width) / height / 2
        this.orthographicCamera.right = (span * width) / height / 2
        this.orthographicCamera.top = span / 2
        this.orthographicCamera.bottom = -span / 2
        this.orthographicCamera.updateProjectionMatrix()
        this.requestRender()
    }

    private readonly handlePointerDown = (event: PointerEvent) => {
        if (this.viewMode === 'walk') return
        this.pointerStart = new THREE.Vector2(event.clientX, event.clientY)
    }

    private readonly handlePointerUp = (event: PointerEvent) => {
        if (this.viewMode === 'walk') return
        if (event.button !== 0 || !this.pointerStart) return
        const distance = this.pointerStart.distanceTo(
            new THREE.Vector2(event.clientX, event.clientY),
        )
        this.pointerStart = undefined
        if (distance > 4) return
        const rect = this.canvas.getBoundingClientRect()
        this.pointer.set(
            ((event.clientX - rect.left) / rect.width) * 2 - 1,
            -((event.clientY - rect.top) / rect.height) * 2 + 1,
        )
        this.raycaster.setFromCamera(this.pointer, this.activeCamera)
        const candidates: THREE.Object3D[] = []
        for (const region of this.regions.values()) {
            if (region.visible) candidates.push(...region.children)
        }
        const intersections = this.raycaster.intersectObjects(candidates, true)
        const intersection = intersections.find((item) => {
            if (
                !(item.object instanceof THREE.Mesh) ||
                !item.object.visible ||
                item.faceIndex === undefined
            ) {
                return false
            }
            const attribute = item.object.geometry.getAttribute('blockPosition')
            const blockY = attribute.getY(item.faceIndex * 3)
            return (
                !this.layerRange || (blockY >= this.layerRange[0] && blockY <= this.layerRange[1])
            )
        })
        if (!intersection || !(intersection.object instanceof THREE.Mesh)) {
            this.onSelect()
            return
        }
        const attribute = intersection.object.geometry.getAttribute('blockPosition')
        const vertexIndex = (intersection.faceIndex ?? 0) * 3
        const selection: SchematicSceneSelection = {
            regionId: intersection.object.userData.regionId as string,
            position: [
                Math.round(attribute.getX(vertexIndex)),
                Math.round(attribute.getY(vertexIndex)),
                Math.round(attribute.getZ(vertexIndex)),
            ],
        }
        this.onSelect(selection)
        if (event.detail >= 2) this.onFocus(selection)
    }

    private readonly handleWalkKeyDown = (event: KeyboardEvent) => {
        if (this.viewMode !== 'walk') return
        if (event.code === 'Escape' && this.nativeWalkLocked) {
            event.preventDefault()
            void this.releaseNativeWalkLock()
            return
        }
        if (this.requireWalkLock && !this.walkControls.isLocked && !this.nativeWalkLocked) return
        if (
            ['KeyW', 'KeyA', 'KeyS', 'KeyD', 'Space', 'ShiftLeft', 'ShiftRight'].includes(
                event.code,
            )
        ) {
            event.preventDefault()
            this.pressedKeys.add(event.code)
        }
    }

    private readonly handleWalkKeyUp = (event: KeyboardEvent) => {
        this.pressedKeys.delete(event.code)
    }

    private readonly handleWalkBlur = () => {
        this.pressedKeys.clear()
        void this.releaseNativeWalkLock()
    }

    private readonly handleWalkClick = (event: MouseEvent) => {
        if (this.requireWalkLock && event.target !== this.canvas) return
        if (this.viewMode === 'walk') this.requestWalkLock()
    }

    private readonly handleWalkMouseMove = (event: MouseEvent) => {
        if (this.viewMode !== 'walk' || !this.nativeWalkLocked) return
        const movementX = filterNativeWalkMouseDelta(event.movementX)
        const movementY = filterNativeWalkMouseDelta(event.movementY)
        if (movementX === 0 && movementY === 0) return
        this.walkEuler.setFromQuaternion(this.perspectiveCamera.quaternion)
        this.walkEuler.y -= movementX * 0.002
        this.walkEuler.x -= movementY * 0.002
        this.walkEuler.x = Math.max(-Math.PI / 2, Math.min(Math.PI / 2, this.walkEuler.x))
        this.perspectiveCamera.quaternion.setFromEuler(this.walkEuler)
        this.requestRender()
    }

    private readonly handleWalkPointerLockError = () => {
        if (this.viewMode !== 'walk' || this.nativeWalkLocked || !this.onNativeWalkLock) return
        void this.requestNativeWalkLock()
    }

    private readonly handleWalkWheel = (event: WheelEvent) => {
        if (this.viewMode !== 'walk' || (!this.walkControls.isLocked && !this.nativeWalkLocked)) {
            return
        }
        event.preventDefault()
        event.stopPropagation()
        if (event.deltaY === 0) return
        const multiplier = event.deltaY > 0 ? WALK_SPEED_MULTIPLIER : 1 / WALK_SPEED_MULTIPLIER
        this.walkSpeed = Math.round(
            Math.max(WALK_SPEED_MINIMUM, Math.min(WALK_SPEED_MAXIMUM, this.walkSpeed * multiplier)),
        )
        this.onWalkSpeedChange(this.walkSpeed)
    }

    private readonly handleWalkLock = () => {
        if (this.viewMode === 'walk') this.onWalkLockChange(true)
    }

    private readonly handleWalkUnlock = () => {
        this.pressedKeys.clear()
        if (!this.nativeWalkLocked) this.onWalkLockChange(false)
    }

    private requestWalkLock() {
        if (
            this.viewMode !== 'walk' ||
            this.walkControls.isLocked ||
            this.nativeWalkLocked ||
            this.nativeWalkLockPending
        ) {
            return
        }
        if (this.onNativeWalkLock) {
            void this.requestNativeWalkLock().then((locked) => {
                if (!locked && this.viewMode === 'walk' && !this.nativeWalkLocked) {
                    this.requestBrowserWalkLock()
                }
            })
            return
        }
        this.requestBrowserWalkLock()
    }

    private requestBrowserWalkLock() {
        if (this.viewMode !== 'walk' || this.walkControls.isLocked || this.nativeWalkLocked) return
        try {
            this.walkControls.lock()
        } catch {
            this.onWalkLockChange(false)
        }
    }

    private async requestNativeWalkLock() {
        if (!this.onNativeWalkLock || this.nativeWalkLockPending || this.nativeWalkLocked) {
            return false
        }
        this.nativeWalkLockPending = true
        try {
            const locked = await this.onNativeWalkLock()
            if (!locked || this.disposed || this.viewMode !== 'walk') {
                if (locked) await this.onNativeWalkUnlock?.()
                return false
            }
            this.nativeWalkLocked = true
            this.onWalkLockChange(true)
            return true
        } catch {
            return false
        } finally {
            this.nativeWalkLockPending = false
        }
    }

    private async releaseNativeWalkLock() {
        if (!this.nativeWalkLocked) return
        this.nativeWalkLocked = false
        this.onWalkLockChange(false)
        try {
            await this.onNativeWalkUnlock?.()
        } catch {
            return
        }
    }

    private startWalkLoop() {
        if (this.walkFrame !== undefined) return
        const update = (time: number) => {
            if (this.disposed) {
                this.walkFrame = undefined
                return
            }
            const delta = (time - this.walkPreviousTime) / 1000
            this.walkPreviousTime = time
            const targetFov = this.viewMode === 'walk' ? WALK_FOV : ORBIT_FOV
            const fovDifference = targetFov - this.perspectiveCamera.fov
            if (Math.abs(fovDifference) > 0.05) {
                this.perspectiveCamera.fov += fovDifference * 0.15
                this.perspectiveCamera.updateProjectionMatrix()
            } else if (this.perspectiveCamera.fov !== targetFov) {
                this.perspectiveCamera.fov = targetFov
                this.perspectiveCamera.updateProjectionMatrix()
            }
            if (
                this.viewMode === 'walk' &&
                (!this.requireWalkLock || this.walkControls.isLocked || this.nativeWalkLocked)
            ) {
                const forward =
                    Number(this.pressedKeys.has('KeyW')) - Number(this.pressedKeys.has('KeyS'))
                const right =
                    Number(this.pressedKeys.has('KeyD')) - Number(this.pressedKeys.has('KeyA'))
                const vertical =
                    Number(this.pressedKeys.has('Space')) -
                    Number(this.pressedKeys.has('ShiftLeft') || this.pressedKeys.has('ShiftRight'))
                if (forward !== 0 || right !== 0 || vertical !== 0) {
                    this.perspectiveCamera.getWorldDirection(this.walkForward)
                    this.walkForward.y = 0
                    this.walkForward.normalize()
                    this.walkRight
                        .crossVectors(this.walkForward, this.perspectiveCamera.up)
                        .normalize()
                    this.walkDirection.set(0, 0, 0)
                    this.walkDirection.addScaledVector(this.walkForward, forward)
                    this.walkDirection.addScaledVector(this.walkRight, right)
                    this.walkDirection.y += vertical
                    this.walkDirection.normalize()
                    this.perspectiveCamera.position.addScaledVector(
                        this.walkDirection,
                        this.walkSpeed * delta,
                    )
                }
            }
            this.render()
            if (this.viewMode === 'walk' || this.perspectiveCamera.fov !== targetFov) {
                this.walkFrame = requestAnimationFrame(update)
            } else {
                this.walkFrame = undefined
            }
        }
        this.walkFrame = requestAnimationFrame(update)
    }

    private readonly handleContextLost = (event: Event) => {
        event.preventDefault()
        if (!this.disposed) this.onContextLost()
    }

    private readonly handleContextRestored = () => {
        if (this.disposed) return
        this.onContextRestored()
        this.requestRender()
    }

    private startControlLoop() {
        if (this.controlFrame !== undefined) return
        const update = () => {
            this.controlFrame = undefined
            if (this.disposed || !this.controls.update()) return
            this.render()
            this.controlFrame = requestAnimationFrame(update)
        }
        this.controlFrame = requestAnimationFrame(update)
    }

    private requestRender() {
        if (this.disposed || this.renderFrame !== undefined) return
        this.renderFrame = requestAnimationFrame(() => {
            this.renderFrame = undefined
            if (this.viewMode === 'orbit') this.controls.update()
            this.render()
        })
    }

    private render() {
        if (!this.disposed) this.renderer.render(this.scene, this.activeCamera)
    }

    private disposeObject(object: THREE.Object3D) {
        object.traverse((child) => {
            if (child instanceof THREE.Mesh) {
                child.geometry.dispose()
                for (const name of Object.keys(child.geometry.attributes))
                    child.geometry.deleteAttribute(name)
                child.geometry.setIndex(null)
                delete child.userData.basePositions
            }
        })
    }

    private disposeHelper(object: THREE.Object3D) {
        const helper = object as THREE.Object3D & {
            geometry?: THREE.BufferGeometry
            material?: { dispose: () => void } | Array<{ dispose: () => void }>
        }
        helper.geometry?.dispose()
        if (Array.isArray(helper.material)) {
            for (const material of helper.material) material.dispose()
        } else {
            helper.material?.dispose()
        }
    }
}
