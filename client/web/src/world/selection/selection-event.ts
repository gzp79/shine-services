import type { Chunk } from '../chunk';
import type { ChunkCorner } from '../chunk-corner';
import type { ChunkEdge } from '../chunk-edge';
import type { ChunkId } from '../chunk-id';

export type Selection =
    | { type: 'cell'; chunk: Chunk; cellId: number }
    | { type: 'edge-cell'; edge: ChunkEdge; cellId: number }
    | { type: 'corner-cell'; corner: ChunkCorner; cellId: number };

export namespace Selection {
    export function owner(sel: Selection): Chunk | ChunkEdge | ChunkCorner {
        return sel.type === 'cell' ? sel.chunk : sel.type === 'edge-cell' ? sel.edge : sel.corner;
    }

    export function isInteractable(sel: Selection, reference: ChunkId): boolean {
        return sel.type === 'cell'
            ? sel.chunk.id.isInteractable(reference)
            : sel.type === 'edge-cell'
              ? sel.edge.id.isInteractable(reference)
              : sel.corner.id.isInteractable(reference);
    }
}

export const SELECTION_CHANGED = 'selectionchanged';

export type SelectionChangedEvent = {
    selection: Selection | null;
};
