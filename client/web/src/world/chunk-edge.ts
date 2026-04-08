import { WasmWorld } from '#wasm';
import * as THREE from 'three';
import { PolygonData } from '../engine/mesh/geometry-data';
import { PolygonWireMesh } from '../engine/mesh/polygon-wire-mesh';
import { SelectionMesh } from '../engine/mesh/selection-mesh';
import { ChunkId } from './chunk-id';

export class ChunkEdgeId {
    constructor(
        public readonly chunkId: ChunkId,
        public readonly edgeIdx: 0 | 1 | 2
    ) {}

    key(): string {
        return `${this.chunkId.key()}-e${this.edgeIdx}`;
    }

    equals(other: ChunkEdgeId): boolean {
        return this.chunkId.equals(other.chunkId) && this.edgeIdx === other.edgeIdx;
    }

    neighborChunkId(): ChunkId {
        return this.chunkId.neighbor(this.edgeIdx);
    }
}

export class ChunkEdge {
    readonly group = new THREE.Group();
    private wireframe: PolygonWireMesh;
    private selection: SelectionMesh;

    constructor(
        private readonly world: WasmWorld,
        readonly id: ChunkEdgeId,
        private readonly events: EventTarget
    ) {
        this.group.userData = { chunkEdgeId: id, chunkEdge: this };
        const polygonData = new PolygonData(new Float32Array(), new Uint32Array(), new Uint32Array([]));
        this.selection = new SelectionMesh(this.group, polygonData);
        this.wireframe = new PolygonWireMesh(this.group, polygonData);
    }

    init(referenceChunkId: ChunkId): void {
        const offset = this.worldOffset(referenceChunkId);
        this.group.position.set(offset[0], offset[1], 0);
    }

    worldOffset(ref: ChunkId): Float32Array {
        return this.world.chunk_world_offset(ref.q, ref.r, this.id.chunkId.q, this.id.chunkId.r);
    }

    /*showSelectionAt(worldPos: THREE.Vector3): { cellId: number; localPos: THREE.Vector3 } | null {
        const localPos = this.group.worldToLocal(worldPos.clone());
        const vertIdx = this.findClosestVertex(localPos, this.selection.vertIdx);
        if (vertIdx === -1) {
            this.hideSelection();
            return null;
        }
        this.selection.showAt(vertIdx);
        return { cellId: vertIdx, localPos };
    }*/

    hideSelection(): void {
        this.selection.hide();
    }

    get showPolygonWire(): boolean {
        return this.wireframe.isVisible;
    }

    set showPolygonWire(value: boolean) {
        if (value) this.wireframe.show();
        else this.wireframe.hide();
    }

    dispose(): void {
        this.selection.dispose();
        this.wireframe.dispose();
    }

    /*private findClosestVertex(localPoint: THREE.Vector3, currentVertIdx: number): number {
        // TODO: Port exact logic from Chunk.findClosestVertex for consistent behavior
        // Simplified implementation for now
        if (this.polygonData.vertices.length === 0) return -1;

        // Find closest vertex by checking distance to each polygon center
        let closestIdx = 0;
        let closestDist = Infinity;

        for (let vi = 0; vi < this.polygonData.starts.length - 1; vi++) {
            const start = this.polygonData.starts[vi];
            const end = this.polygonData.starts[vi + 1];
            if (end <= start) continue;

            // Get polygon center (average of vertices)
            let cx = 0,
                cy = 0;
            let count = 0;
            for (let i = start; i < end; i++) {
                const idx = this.polygonData.indices[i];
                cx += this.polygonData.vertices[idx * 2];
                cy += this.polygonData.vertices[idx * 2 + 1];
                count++;
            }
            if (count > 0) {
                cx /= count;
                cy /= count;
                const dx = cx - localPoint.x;
                const dy = cy - localPoint.y;
                const dist = dx * dx + dy * dy;
                if (dist < closestDist) {
                    closestDist = dist;
                    closestIdx = vi;
                }
            }
        }

        // Hysteresis: only switch if significantly closer
        if (currentVertIdx >= 0 && closestDist > 100) {
            return currentVertIdx;
        }

        return closestDist < 1000 ? closestIdx : -1;
    }*/
}
