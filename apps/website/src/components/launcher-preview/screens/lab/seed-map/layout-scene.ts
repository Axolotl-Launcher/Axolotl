import * as THREE from '../../../lib/three.ts'
import { OrbitControls } from '../../../lib/three-orbit-controls.ts'
import type { SeedMapStructurePiece } from './advanced.ts'
import { type PreviewAxis, seedMapPreviewAxes } from './preview-overlays.ts'
import type { StructureBlocksResult, StructureChest } from './structure-blocks.ts'
import { seedMapLayoutPalette } from './structure-layout.ts'

/** On-demand preview sharing one camera between bounds and template block geometry. */
export class SeedMapLayoutScene {
    private readonly renderer: THREE.WebGLRenderer
    private readonly scene = new THREE.Scene()
    private readonly camera = new THREE.PerspectiveCamera(50, 1, 0.1, 10000)
    private readonly controls: OrbitControls
    private readonly geometry = new THREE.BoxGeometry(1, 1, 1)
    private readonly material = new THREE.MeshBasicMaterial({
        transparent: true,
        opacity: 0.22,
        depthWrite: false,
    })
    private readonly outlineMaterial = new THREE.LineBasicMaterial()
    private readonly selectionMaterial = new THREE.LineBasicMaterial()
    private readonly selection: THREE.LineSegments
    private readonly mesh: THREE.InstancedMesh
    private readonly edges: THREE.LineSegments
    private readonly observer: ResizeObserver
    private readonly raycaster = new THREE.Raycaster()
    private readonly pointer = new THREE.Vector2()
    private readonly transforms: THREE.Matrix4[] = []
    private readonly center: THREE.Vector3
    private readonly radius: number
    private start?: { x: number; y: number }
    private frame?: number
    private disposed = false
    private readonly themeObserver: MutationObserver
    private readonly blocks = new THREE.Group()
    private readonly blockMaterial = new THREE.MeshLambertMaterial({
        vertexColors: true,
        side: THREE.DoubleSide,
    })
    private readonly selectedBlockMaterial = new THREE.MeshLambertMaterial({
        side: THREE.DoubleSide,
    })
    private selectedId?: number
    private readonly chests = new THREE.Group()
    private readonly chestGeometry = new THREE.BoxGeometry(1.04, 1.04, 1.04)
    private readonly chestMaterial = new THREE.MeshBasicMaterial({
        transparent: true,
        opacity: 0.22,
        depthTest: false,
        depthWrite: false,
    })
    private readonly chestOutlineMaterial = new THREE.LineBasicMaterial({
        depthTest: false,
        depthWrite: false,
    })
    private chestTargets: StructureChest[] = []
    private chestBounds: THREE.Box3[] = []
    private hoveredChest?: StructureChest

    constructor(
        private readonly canvas: HTMLCanvasElement,
        private readonly pieces: SeedMapStructurePiece[],
        private readonly onSelect: (piece: SeedMapStructurePiece) => void,
        private readonly onOrientation?: (axes: PreviewAxis[]) => void,
        private readonly onChestHover?: (
            chest: StructureChest | undefined,
            point?: { x: number; y: number },
        ) => void,
        private readonly onChestClick?: (containerId: number) => void,
    ) {
        this.renderer = new THREE.WebGLRenderer({
            canvas,
            antialias: true,
            alpha: true,
            preserveDrawingBuffer: true,
        })
        this.renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2))
        this.renderer.setClearColor(0, 0)
        const bounds = new THREE.Box3()
        for (const piece of pieces) {
            bounds.expandByPoint(new THREE.Vector3(piece.min!.x, piece.min!.y, piece.min!.z))
            bounds.expandByPoint(
                new THREE.Vector3(piece.max!.x + 1, piece.max!.y + 1, piece.max!.z + 1),
            )
        }
        this.center = bounds.getCenter(new THREE.Vector3())
        this.radius = Math.max(8, bounds.getSize(new THREE.Vector3()).length() / 2)
        this.camera.far = this.radius * 100
        this.mesh = new THREE.InstancedMesh(this.geometry, this.material, pieces.length)
        const edgeGeometry = new THREE.EdgesGeometry(this.geometry)
        const vertices = edgeGeometry.getAttribute('position')
        const lines: number[] = []
        const point = new THREE.Vector3()
        pieces.forEach((piece, index) => {
            const min = piece.min!,
                max = piece.max!
            const transform = new THREE.Matrix4().compose(
                new THREE.Vector3(
                    (min.x + max.x + 1) / 2,
                    (min.y + max.y + 1) / 2,
                    (min.z + max.z + 1) / 2,
                ).sub(this.center),
                new THREE.Quaternion(),
                new THREE.Vector3(max.x - min.x + 1, max.y - min.y + 1, max.z - min.z + 1),
            )
            this.transforms.push(transform)
            this.mesh.setMatrixAt(index, transform)
            for (let i = 0; i < vertices.count; i++) {
                point.fromBufferAttribute(vertices, i).applyMatrix4(transform)
                lines.push(point.x, point.y, point.z)
            }
        })
        edgeGeometry.dispose()
        this.edges = new THREE.LineSegments(new THREE.BufferGeometry(), this.outlineMaterial)
        this.edges.geometry.setAttribute('position', new THREE.Float32BufferAttribute(lines, 3))
        this.selection = new THREE.LineSegments(
            new THREE.EdgesGeometry(this.geometry),
            this.selectionMaterial,
        )
        this.selection.matrixAutoUpdate = false
        this.selection.visible = false
        this.scene.add(this.mesh, this.edges, this.selection)
        this.scene.add(this.chests)
        this.blocks.visible = false
        this.scene.add(this.blocks, new THREE.HemisphereLight(0xffffff, 0x687080, 0.65))
        const light = new THREE.DirectionalLight(0xffffff, 1.2)
        light.position.set(1, 2, 3)
        this.scene.add(light)
        this.controls = new OrbitControls(this.camera, canvas)
        this.controls.screenSpacePanning = true
        this.controls.zoomToCursor = true
        this.controls.minDistance = 1
        this.controls.maxDistance = this.radius * 20
        this.controls.addEventListener('change', this.render)
        this.themeObserver = new MutationObserver(this.updatePalette)
        this.themeObserver.observe(document.documentElement, {
            attributes: true,
            attributeFilter: ['class', 'style'],
        })
        this.updatePalette()
        this.observer = new ResizeObserver(this.resize)
        this.observer.observe(canvas)
        canvas.addEventListener('pointerdown', this.pointerDown)
        canvas.addEventListener('pointerup', this.pointerUp)
        canvas.addEventListener('pointermove', this.pointerMove)
        canvas.addEventListener('pointerleave', this.pointerLeave)
        this.resize()
        this.fit()
    }

    private resize = () => {
        const rect = this.canvas.getBoundingClientRect()
        this.renderer.setSize(Math.max(1, rect.width), Math.max(1, rect.height), false)
        this.camera.aspect = Math.max(1, rect.width) / Math.max(1, rect.height)
        this.camera.updateProjectionMatrix()
        this.render()
    }

    private updatePalette = () => {
        const palette = seedMapLayoutPalette(this.canvas)
        this.outlineMaterial.color.set(palette.normal)
        this.selectionMaterial.color.set(palette.selected)
        this.selectedBlockMaterial.color.set(palette.selected)
        this.chestMaterial.color.set(palette.selected)
        this.chestOutlineMaterial.color.set(palette.selected)
        this.pieces.forEach((_, index) =>
            this.mesh.setColorAt(
                index,
                new THREE.Color(this.pieces[index].endPortalRoom ? palette.portal : palette.normal),
            ),
        )
        if (this.mesh.instanceColor) this.mesh.instanceColor.needsUpdate = true
        this.render()
    }

    private render = () => {
        if (this.disposed || this.frame !== undefined) return
        this.frame = requestAnimationFrame(() => {
            this.frame = undefined
            this.renderer.render(this.scene, this.camera)
            this.onOrientation?.(seedMapPreviewAxes(this.camera.quaternion))
        })
    }

    fit() {
        const vertical = THREE.MathUtils.degToRad(this.camera.fov / 2)
        const horizontal = Math.atan(Math.tan(vertical) * this.camera.aspect)
        const distance = (this.radius / Math.sin(Math.min(vertical, horizontal))) * 1.15
        this.camera.position.set(distance * 0.6, distance * 0.55, distance * 0.6)
        this.controls.target.set(0, 0, 0)
        this.controls.update()
        this.render()
    }

    select(id?: number) {
        this.selectedId = id
        const index = this.pieces.findIndex((piece) => piece.id === id)
        this.selection.visible = index >= 0
        if (index >= 0) this.selection.matrix.copy(this.transforms[index])
        for (const child of this.blocks.children) {
            const mesh = child as THREE.Mesh
            mesh.material =
                mesh.userData.pieceId === id ? this.selectedBlockMaterial : this.blockMaterial
        }
        this.render()
    }

    setBlocks(result: StructureBlocksResult) {
        this.clearBlocks()
        this.blocks.position.set(...result.origin).sub(this.center)
        for (const item of result.meshes) {
            const geometry = new THREE.BufferGeometry()
            geometry.setAttribute('position', new THREE.BufferAttribute(item.positions, 3))
            geometry.setAttribute('color', new THREE.BufferAttribute(item.colors, 3))
            geometry.computeVertexNormals()
            geometry.computeBoundingSphere()
            const mesh = new THREE.Mesh(geometry, this.blockMaterial)
            mesh.userData.pieceId = item.pieceId
            this.blocks.add(mesh)
        }
        this.select(this.selectedId)
    }

    showBlocks(show: boolean) {
        this.blocks.visible = show
        this.mesh.visible = !show
        this.edges.visible = !show
        this.render()
    }

    setChests(chests: StructureChest[]) {
        this.clearChests()
        this.chestTargets = chests
        this.chestBounds = chests.map((chest) => {
            const center = new THREE.Vector3(
                chest.position[0] - this.center.x + 0.5,
                chest.position[1] - this.center.y + 0.5,
                chest.position[2] - this.center.z + 0.5,
            )
            return new THREE.Box3(center.clone().addScalar(-0.52), center.clone().addScalar(0.52))
        })
        this.hoveredChest = undefined
        this.onChestHover?.(undefined)
        if (!chests.length) {
            this.render()
            return
        }
        const fill = new THREE.InstancedMesh(this.chestGeometry, this.chestMaterial, chests.length)
        fill.renderOrder = 1000
        const edgeGeometry = new THREE.EdgesGeometry(this.chestGeometry)
        const vertices = edgeGeometry.getAttribute('position')
        const lines: number[] = []
        const point = new THREE.Vector3()
        const matrix = new THREE.Matrix4()
        chests.forEach((chest, index) => {
            matrix.makeTranslation(
                chest.position[0] - this.center.x + 0.5,
                chest.position[1] - this.center.y + 0.5,
                chest.position[2] - this.center.z + 0.5,
            )
            fill.setMatrixAt(index, matrix)
            for (let i = 0; i < vertices.count; i++) {
                point.fromBufferAttribute(vertices, i).applyMatrix4(matrix)
                lines.push(point.x, point.y, point.z)
            }
        })
        fill.computeBoundingSphere()
        edgeGeometry.dispose()
        const edges = new THREE.LineSegments(new THREE.BufferGeometry(), this.chestOutlineMaterial)
        edges.geometry.setAttribute('position', new THREE.Float32BufferAttribute(lines, 3))
        edges.renderOrder = 1001
        this.chests.add(fill, edges)
        this.render()
    }

    showChests(show: boolean) {
        this.chests.visible = show
        if (!show) this.pointerLeave()
        this.render()
    }

    private clearChests() {
        for (const child of this.chests.children) {
            if (child instanceof THREE.InstancedMesh) child.dispose()
            else if (child instanceof THREE.LineSegments) child.geometry.dispose()
        }
        this.chests.clear()
    }

    private clearBlocks() {
        for (const child of this.blocks.children) (child as THREE.Mesh).geometry.dispose()
        this.blocks.clear()
    }

    private pointerDown = (event: PointerEvent) => {
        this.start = { x: event.clientX, y: event.clientY }
    }
    private pointerMove = (event: PointerEvent) => {
        if (!this.chests.visible || !this.chestTargets.length) {
            this.pointerLeave()
            return
        }
        const rect = this.canvas.getBoundingClientRect()
        const index = this.chestIndexAtPointer(event)
        const chest = index === undefined ? undefined : this.chestTargets[index]
        if (chest !== this.hoveredChest) {
            this.hoveredChest = chest
            this.onChestHover?.(chest, {
                x: event.clientX - rect.left,
                y: event.clientY - rect.top,
            })
        } else if (chest) {
            this.onChestHover?.(chest, {
                x: event.clientX - rect.left,
                y: event.clientY - rect.top,
            })
        }
    }
    private pointerLeave = () => {
        this.hoveredChest = undefined
        this.onChestHover?.(undefined)
    }
    private chestIndexAtPointer(event: PointerEvent) {
        const rect = this.canvas.getBoundingClientRect()
        if (!rect.width || !rect.height) return undefined
        this.pointer.set(
            ((event.clientX - rect.left) / rect.width) * 2 - 1,
            (-(event.clientY - rect.top) / rect.height) * 2 + 1,
        )
        this.camera.updateMatrixWorld()
        this.raycaster.setFromCamera(this.pointer, this.camera)
        const point = new THREE.Vector3()
        let nearest = Number.POSITIVE_INFINITY
        let index: number | undefined
        this.chestBounds.forEach((bounds, candidate) => {
            const hit = this.raycaster.ray.intersectBox(bounds, point)
            if (!hit) return
            const distance = this.raycaster.ray.origin.distanceToSquared(hit)
            if (distance >= nearest) return
            nearest = distance
            index = candidate
        })
        return index
    }
    private pointerUp = (event: PointerEvent) => {
        if (
            event.button !== 0 ||
            !this.start ||
            Math.hypot(event.clientX - this.start.x, event.clientY - this.start.y) > 4
        )
            return
        this.start = undefined
        if (this.chests.visible) {
            const chestIndex = this.chestIndexAtPointer(event)
            const chest = chestIndex === undefined ? undefined : this.chestTargets[chestIndex]
            if (chest?.containerId !== undefined) {
                this.onChestClick?.(chest.containerId)
                return
            }
        }
        const rect = this.canvas.getBoundingClientRect()
        if (!rect.width || !rect.height) return
        this.pointer.set(
            ((event.clientX - rect.left) / rect.width) * 2 - 1,
            (-(event.clientY - rect.top) / rect.height) * 2 + 1,
        )
        this.camera.updateMatrixWorld()
        this.raycaster.setFromCamera(this.pointer, this.camera)
        if (this.blocks.visible) {
            const hit = this.raycaster.intersectObjects(this.blocks.children)[0]
            const piece = this.pieces.find((piece) => piece.id === hit?.object.userData.pieceId)
            if (piece) this.onSelect(piece)
        } else {
            const hit = this.raycaster.intersectObject(this.mesh)[0]
            if (hit?.instanceId !== undefined) this.onSelect(this.pieces[hit.instanceId])
        }
    }

    dispose() {
        this.disposed = true
        if (this.frame !== undefined) cancelAnimationFrame(this.frame)
        this.observer.disconnect()
        this.themeObserver.disconnect()
        this.controls.dispose()
        this.canvas.removeEventListener('pointerdown', this.pointerDown)
        this.canvas.removeEventListener('pointerup', this.pointerUp)
        this.canvas.removeEventListener('pointermove', this.pointerMove)
        this.canvas.removeEventListener('pointerleave', this.pointerLeave)
        this.mesh.dispose()
        this.geometry.dispose()
        this.edges.geometry.dispose()
        this.material.dispose()
        this.outlineMaterial.dispose()
        this.selectionMaterial.dispose()
        this.selection.geometry.dispose()
        this.clearBlocks()
        this.blockMaterial.dispose()
        this.selectedBlockMaterial.dispose()
        this.clearChests()
        this.chestGeometry.dispose()
        this.chestMaterial.dispose()
        this.chestOutlineMaterial.dispose()
        this.renderer.dispose()
    }
}
