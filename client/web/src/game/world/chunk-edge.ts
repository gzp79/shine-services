import { EdgeCells, World } from '#wasm';
import * as THREE from 'three';
import { EventSubscriptions } from '../../engine/events';
import { SelectionMesh } from '../../engine/scene/selection-mesh';
import { computeLocalCentroids } from '../../mesh/centroid';
import { asPolygonMesh } from '../../mesh/polygon-mesh';
import { ChunkId, HexFlatDir } from './chunk-id';
import type { WorldEntity, WorldEntityKind } from './world-entity';
import { SELECTION_CHANGED, type SelectionChangedEvent } from './world-events';

export class ChunkEdgeId {
    constructor(
        // The "owner" chunk id. Edge data is stored relative to this chunk.
        public readonly chunkId: ChunkId,
        public readonly edgeIdx: HexFlatDir.NE | HexFlatDir.N | HexFlatDir.NW
    ) {}

    get kind(): WorldEntityKind {
        return 'edge';
    }

    key(): string {
        return `${this.chunkId.key()}-e${this.edgeIdx}`;
    }

    equals(other: ChunkEdgeId): boolean {
        return this.chunkId.equals(other.chunkId) && this.edgeIdx === other.edgeIdx;
    }

    involvedChunkIds(): [ChunkId, ChunkId] {
        return [this.chunkId, this.chunkId.neighbor(this.edgeIdx)];
    }

    isInteractable(reference: ChunkId): boolean {
        return this.involvedChunkIds().some((id) => id.isInteractable(reference));
    }
}

export class ChunkEdge implements WorldEntity {
    readonly group = new THREE.Group();
    readonly cells: EdgeCells;
    private selectionMesh: SelectionMesh;
    private _centroids: Float32Array | null = null;
    private readonly subscriptions: EventSubscriptions;

    constructor(
        private readonly world: World,
        readonly id: ChunkEdgeId,
        events: EventTarget
    ) {
        this.group.userData = { chunkEdgeId: id, chunkEdge: this };
        this.cells = world.edge_cells(id.chunkId.q, id.chunkId.r, id.edgeIdx)!;
        this.selectionMesh = new SelectionMesh(this.group, asPolygonMesh(this.cells));
        this.subscriptions = new EventSubscriptions(events);
        this.subscriptions.on<SelectionChangedEvent>(SELECTION_CHANGED, this.handleSelectionChanged);
    }

    init(referenceChunkId: ChunkId): void {
        const offset = this.worldOffset(referenceChunkId);
        this.group.position.set(offset[0], offset[1], 0);
    }

    worldOffset(ref: ChunkId): [number, number] {
        return this.world.chunk_world_offset(ref.q, ref.r, this.id.chunkId.q, this.id.chunkId.r);
    }

    get centroids(): Float32Array {
        if (!this._centroids) {
            this._centroids = computeLocalCentroids(asPolygonMesh(this.cells))!;
        }
        return this._centroids;
    }

    dispose(): void {
        this.subscriptions.dispose();
        this.selectionMesh.dispose();
        this.cells.free();
    }

    private handleSelectionChanged = (event: SelectionChangedEvent): void => {
        const sel = event.selection;
        if (sel?.type === 'edge-cell' && sel.edge === this) {
            this.selectionMesh.show(sel.cellId);
        } else {
            this.selectionMesh.hide();
        }
    };
}
