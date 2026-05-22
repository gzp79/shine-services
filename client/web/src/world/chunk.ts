import { InnerCellsHandle, WasmWorld } from '#wasm';
import * as THREE from 'three';
import { EventSubscriptions } from '../engine/events';
import { PolygonWireMesh } from '../engine/mesh/polygon-wire-mesh';
import { ChunkId } from './chunk-id';

export class InternalCell {
    readonly position: THREE.Vector3;
    readonly index: number;

    constructor(index: number, x: number, y: number) {
        this.index = index;
        this.position = new THREE.Vector3(x, y, 0);
    }
}

export class Chunk {
    readonly group = new THREE.Group();
    readonly innerCells: InnerCellsHandle;

    private label: THREE.Sprite | null = null;
    private cellWires: PolygonWireMesh | null = null;
    private subscription: EventSubscriptions;

    constructor(
        private readonly world: WasmWorld,
        readonly id: ChunkId,
        private readonly events: EventTarget
    ) {
        this.group.userData = { chunkId: { q: id.q, r: id.r }, chunk: this };
        this.innerCells = world.inner_cells(id.q, id.r)!;
        //this.selection = new SelectionMesh(this.group, dualPolygons);

        // Subscribe to focus change events
        this.subscription = new EventSubscriptions(events);
    }

    init(referenceChunkId: ChunkId): void {
        const offset = this.worldOffset(referenceChunkId);
        this.group.position.set(offset[0], offset[1], 0);

        if (this.showCellWires) {
            this.buildCellWires();
        }
    }

    dispose(): void {
        //this.selection.dispose();
        this.disposeLabel();
        this.cellWires?.dispose();
        this.innerCells.free();
    }

    get showLabel(): boolean {
        return this.label?.visible ?? false;
    }

    set showLabel(value: boolean) {
        if (value) {
            this.createLabel();
        } else {
            this.disposeLabel();
        }
    }

    get showCellWires(): boolean {
        return this.cellWires?.isVisible() ?? false;
    }

    set showCellWires(value: boolean) {
        if (value) {
            this.buildCellWires();
        } else {
            this.cellWires?.dispose();
            this.cellWires = null;
        }
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
    }

    hideSelection(): void {
        this.selection.hide();
    }*/

    worldOffset(ref: ChunkId): [number, number] {
        return this.world.chunk_world_offset(ref.q, ref.r, this.id.q, this.id.r);
    }

    /*private findClosestVertex(localPoint: THREE.Vector3, currentVertIdx: number): number {
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
    }*/

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

    private disposeLabel(): void {
        if (!this.label) return;

        this.group.remove(this.label);
        const material = this.label.material as THREE.SpriteMaterial;
        material.map?.dispose();
        material.dispose();
        this.label = null;
    }

    private buildCellWires(): void {
        if (this.cellWires !== null) {
            return;
        }

        this.cellWires = PolygonWireMesh.fromPolygons(this.group, this.innerCells);
        this.cellWires.show();
    }
}
