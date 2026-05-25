import * as THREE from 'three';
import type { Chunk } from '../chunk';
import type { ChunkCorner } from '../chunk-corner';
import type { ChunkEdge } from '../chunk-edge';

export type Selection =
    | { type: 'cell'; chunk: Chunk; cellId: number; centroid: THREE.Vector2 }
    | { type: 'edge-cell'; edge: ChunkEdge; cellId: number; centroid: THREE.Vector2 }
    | { type: 'corner-cell'; corner: ChunkCorner; cellId: number; centroid: THREE.Vector2 };

export const SELECTION_CHANGED = 'selectionchanged';

export type SelectionChangedEvent = {
    selection: Selection | null;
};
