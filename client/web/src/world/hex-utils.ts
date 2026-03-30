import * as THREE from 'three';
import { ChunkId } from './chunk-id';

const CHUNK_WORLD_SIZE = 1000;

/**
 * Convert world coordinates to the ChunkId containing that position.
 * World coordinates are relative to referenceChunkId being at origin.
 * Uses flat-top hex grid (matching Rust implementation).
 */
export function worldPositionToChunkId(referenceChunkId: ChunkId, worldPos: THREE.Vector2): ChunkId {
    const sqrt3 = Math.sqrt(3);
    const size = CHUNK_WORLD_SIZE;

    // Get absolute position of reference chunk
    const refX = size * (3 / 2) * referenceChunkId.q;
    const refY = size * ((sqrt3 / 2) * referenceChunkId.q + sqrt3 * referenceChunkId.r);

    // Convert world position (relative to reference) to absolute
    const absX = worldPos.x + refX;
    const absY = worldPos.y + refY;

    // Convert absolute position to hex coordinates
    const q = (2 / 3) * (absX / size);
    const r = absY / size / sqrt3 - q / 2;

    // Round to nearest hex using cube coordinates
    return roundToHex(q, r);
}

/**
 * Convert ChunkId to its world position (hex center).
 * Returns position relative to referenceChunkId being at origin.
 */
export function chunkIdToWorldPosition(referenceChunkId: ChunkId, chunkId: ChunkId): THREE.Vector2 {
    const sqrt3 = Math.sqrt(3);
    const size = CHUNK_WORLD_SIZE;

    // Calculate absolute positions
    const refX = size * (3 / 2) * referenceChunkId.q;
    const refY = size * ((sqrt3 / 2) * referenceChunkId.q + sqrt3 * referenceChunkId.r);

    const chunkX = size * (3 / 2) * chunkId.q;
    const chunkY = size * ((sqrt3 / 2) * chunkId.q + sqrt3 * chunkId.r);

    // Return position relative to reference
    return new THREE.Vector2(chunkX - refX, chunkY - refY);
}

/**
 * Round fractional axial coordinates to nearest hex.
 * Uses cube coordinate rounding algorithm.
 */
function roundToHex(q: number, r: number): ChunkId {
    const s = -q - r;

    let rq = Math.round(q);
    let rr = Math.round(r);
    const rs = Math.round(s);

    const q_diff = Math.abs(rq - q);
    const r_diff = Math.abs(rr - r);
    const s_diff = Math.abs(rs - s);

    // Reset the coordinate with largest error to maintain q + r + s = 0
    if (q_diff > r_diff && q_diff > s_diff) {
        rq = -rr - rs;
    } else if (r_diff > s_diff) {
        rr = -rq - rs;
    }

    return new ChunkId(rq, rr);
}
