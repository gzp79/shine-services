import { WasmWorld } from '#wasm';
import * as THREE from 'three';
import { EventSubscriptions } from '../engine/events';
import { PolygonData, QuadData } from '../engine/mesh/geometry-data';
import { MeshBuilder, buildBaseMesh } from '../engine/mesh/quad-mesh';
import { SelectionMesh } from '../engine/mesh/selection-mesh';
import { ChunkId } from './chunk-id';

export class Chunk {
    readonly group = new THREE.Group();

    private mesh: MeshBuilder | null = null;
    private label: THREE.Sprite | null = null;
    private selection: SelectionMesh;
    private subscription: EventSubscriptions;

    constructor(
        private readonly world: WasmWorld,
        readonly id: ChunkId,
        private readonly events: EventTarget
    ) {
        this.group.userData = { chunkId: { q: id.q, r: id.r }, chunk: this };
        this.selection = new SelectionMesh(this.group, this.dualPolygons());

        // Subscribe to focus change events
        this.subscription = new EventSubscriptions(events);
    }

    init(referenceChunkId: ChunkId): void {
        const offset = this.worldOffset(referenceChunkId);
        this.group.position.set(offset[0], offset[1], 0);

        this.buildMesh();
    }

    dispose(): void {
        this.disposeMesh();
        this.desposeLabel();
        this.selection.dispose();
    }

    get showLabel(): boolean {
        return this.label?.visible ?? false;
    }

    set showLabel(value: boolean) {
        if (value) {
            this.createLabel();
        } else {
            this.desposeLabel();
        }
    }

    showSelectionAt(worldPoint: THREE.Vector3): void {
        const localPoint = this.group.worldToLocal(worldPoint.clone());
        const vertIdx = this.findClosestVertex(localPoint, this.selection.vertIdx);
        if (vertIdx !== -1) {
            this.selection.showAt(vertIdx);
        }
    }

    hideSelection(): void {
        this.selection.hide();
    }

    quadData(): QuadData {
        return new QuadData(
            this.world.chunk_quad_vertices(this.id.q, this.id.r),
            this.world.chunk_quad_indices(this.id.q, this.id.r),
            this.world.chunk_boundary_indices(this.id.q, this.id.r)
        );
    }

    dualPolygons(): PolygonData {
        const vertices = this.world.chunk_dual_vertices(this.id.q, this.id.r);
        const packed = this.world.chunk_dual_polygons(this.id.q, this.id.r);

        if (packed.length === 0) {
            return new PolygonData(vertices, new Uint32Array(0), new Uint32Array(0));
        }

        const startsLen = packed[0];
        const starts = packed.slice(1, 1 + startsLen);
        const indices = packed.slice(1 + startsLen);

        return new PolygonData(vertices, indices, starts);
    }

    worldOffset(ref: ChunkId): Float32Array {
        return this.world.chunk_world_offset(ref.q, ref.r, this.id.q, this.id.r);
    }

    private findClosestVertex(localPoint: THREE.Vector3, currentVertIdx: number): number {
        const vertices = this.world.chunk_quad_vertices(this.id.q, this.id.r);
        if (vertices.length === 0) return -1;

        let closestIdx = 0;
        let closestDist = Infinity;

        // Calculate distance to current vertex if valid
        let currentDist = Infinity;
        if (currentVertIdx >= 0 && currentVertIdx < vertices.length / 2) {
            const vx = vertices[currentVertIdx * 2];
            const vy = vertices[currentVertIdx * 2 + 1];
            const dx = vx - localPoint.x;
            const dy = vy - localPoint.y;
            currentDist = dx * dx + dy * dy;
        }

        for (let i = 0; i < vertices.length / 2; i++) {
            const vx = vertices[i * 2];
            const vy = vertices[i * 2 + 1];
            const dx = vx - localPoint.x;
            const dy = vy - localPoint.y;
            const dist = dx * dx + dy * dy;
            if (dist < closestDist) {
                closestDist = dist;
                closestIdx = i;
            }
        }

        // Add hysteresis: only switch if new vertex is significantly closer (50% of current distance)
        if (currentVertIdx >= 0 && closestDist > currentDist * 0.25) {
            return currentVertIdx;
        }

        return closestIdx;
    }

    /** Simple hash for deterministic debug color. */
    private chunkHash(): number {
        let h = ((this.id.q | 0) * 0x9e3779b9 + (this.id.r | 0)) >>> 0;
        h ^= h >>> 16;
        h = Math.imul(h, 0x45d9f3b) >>> 0;
        h ^= h >>> 16;
        return h;
    }

    private buildMesh(): void {
        if (this.mesh) return;

        const color = new THREE.Color().setHSL(this.chunkHash() / 0xffffffff, 0.5, 0.6);

        this.mesh = buildBaseMesh(this.quadData(), color);
        this.group.add(this.mesh.group);
    }

    private disposeMesh(): void {
        if (this.mesh) {
            this.group.remove(this.mesh.group);
            this.mesh.dispose();
            this.mesh = null;
        }
        this.desposeLabel();
        this.selection.dispose();
        this.subscription.dispose();
    }

    private createLabel(): void {
        if (this.label) return;

        // Create canvas for text
        const canvas = document.createElement('canvas');
        const context = canvas.getContext('2d')!;
        canvas.width = 256;
        canvas.height = 128;

        // Draw text
        context.fillStyle = 'rgba(0, 0, 0, 0.7)';
        context.fillRect(0, 0, canvas.width, canvas.height);
        context.font = 'bold 48px monospace';
        context.fillStyle = 'white';
        context.textAlign = 'center';
        context.textBaseline = 'middle';
        context.fillText(`(${this.id.q}, ${this.id.r})`, canvas.width / 2, canvas.height / 2);

        // Create sprite
        const texture = new THREE.CanvasTexture(canvas);
        const material = new THREE.SpriteMaterial({
            map: texture,
            depthTest: false,
            depthWrite: false
        });
        this.label = new THREE.Sprite(material);
        this.label.scale.set(200, 100, 1); // Scale in world units
        this.label.position.set(0, 0, 50); // Position above chunk center
        this.label.renderOrder = 998; // Render before centerDot
        this.group.add(this.label);
    }

    private desposeLabel(): void {
        if (!this.label) return;

        this.group.remove(this.label);
        const material = this.label.material as THREE.SpriteMaterial;
        material.map?.dispose();
        material.dispose();
        this.label = null;
    }
}
