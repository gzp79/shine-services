import { InnerCells, World } from '#wasm';
import * as THREE from 'three';
import { EventSubscriptions } from '../../engine/events';
import { SelectionMesh } from '../../engine/scene/selection-mesh';
import { computeLocalCentroids } from '../../mesh/centroid';
import { asPolygonMesh } from '../../mesh/polygon-mesh';
import { ChunkId } from './chunk-id';
import type { WorldEntity } from './world-entity';
import { SELECTION_CHANGED, type SelectionChangedEvent } from './world-events';

export class Chunk implements WorldEntity {
    readonly group = new THREE.Group();
    readonly cells: InnerCells;

    private selectionMesh: SelectionMesh;
    private _centroids: Float32Array | null = null;
    private readonly subscriptions: EventSubscriptions;

    constructor(
        private readonly world: World,
        readonly id: ChunkId,
        events: EventTarget
    ) {
        this.group.userData = { chunkId: { q: id.q, r: id.r }, chunk: this };
        this.cells = world.inner_cells(id.q, id.r)!;
        this.selectionMesh = new SelectionMesh(this.group, asPolygonMesh(this.cells));
        this.subscriptions = new EventSubscriptions(events);
        this.subscriptions.on<SelectionChangedEvent>(SELECTION_CHANGED, this.handleSelectionChanged);
    }

    init(referenceChunkId: ChunkId): void {
        const offset = this.worldOffset(referenceChunkId);
        this.group.position.set(offset[0], offset[1], 0);
    }

    dispose(): void {
        this.subscriptions.dispose();
        this.selectionMesh.dispose();
        this.cells.free();
    }

    worldOffset(ref: ChunkId): [number, number] {
        return this.world.chunk_world_offset(ref.q, ref.r, this.id.q, this.id.r);
    }

    get centroids(): Float32Array {
        if (!this._centroids) {
            this._centroids = computeLocalCentroids(asPolygonMesh(this.cells))!;
        }
        return this._centroids;
    }

    private handleSelectionChanged = (event: SelectionChangedEvent): void => {
        const sel = event.selection;
        if (sel?.type === 'cell' && sel.chunk === this) {
            this.selectionMesh.show(sel.cellId);
        } else {
            this.selectionMesh.hide();
        }
    };
}
