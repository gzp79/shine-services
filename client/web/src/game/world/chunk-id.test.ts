import { hex_distance, hex_flat_from_position, hex_flat_neighbor, hex_flat_to_position, hex_ring } from '#wasm';
import init from '#wasm';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { beforeAll, describe, expect, it } from 'vitest';
import { ChunkConst } from '../../constants';
import { ChunkId, HexFlatDir } from './chunk-id';

beforeAll(async () => {
    const wasmPath = fileURLToPath(new URL('../../../pkg/shine_game_bg.wasm', import.meta.url));
    const wasmModule = new WebAssembly.Module(readFileSync(wasmPath));
    await init(wasmModule);
});

describe('HexFlatDir indices vs WASM', () => {
    it.each([
        [HexFlatDir.NE, 0, 'NE'],
        [HexFlatDir.N, 1, 'N'],
        [HexFlatDir.NW, 2, 'NW'],
        [HexFlatDir.SW, 3, 'SW'],
        [HexFlatDir.S, 4, 'S'],
        [HexFlatDir.SE, 5, 'SE']
    ] as const)('%s (%s) neighbor from origin matches WASM dir %i', (dir, wasmDir, _name) => {
        const js = ChunkId.ORIGIN.neighbor(dir);
        const wasm = hex_flat_neighbor(0, 0, wasmDir);
        expect(js.q).toBe(wasm[0]);
        expect(js.r).toBe(wasm[1]);
    });
});

describe('ChunkId.distanceTo vs WASM hex_distance', () => {
    it.each([
        [0, 0, 0, 0],
        [0, 0, 1, -1],
        [0, 0, -1, 1],
        [0, 0, 0, 1],
        [3, -2, 4, -2],
        [0, 0, -4, 4],
        [0, 0, 2, 2]
    ] as [number, number, number, number][])('distance (%i,%i)→(%i,%i)', (aq, ar, bq, br) => {
        const js = new ChunkId(aq, ar).distanceTo(new ChunkId(bq, br));
        const wasm = hex_distance(aq, ar, bq, br);
        expect(js).toBe(wasm);
    });
});

describe('ChunkId.ring vs WASM hex_ring', () => {
    it.each([0, 1, 2, 3])('ring(%i) from origin order matches WASM', (radius) => {
        const jsRing = ChunkId.ORIGIN.ring(radius);
        const wasmFlat = hex_ring(0, 0, radius);

        expect(jsRing.length).toBe(wasmFlat.length / 2);
        for (let i = 0; i < jsRing.length; i++) {
            expect(jsRing[i].q).toBe(wasmFlat[i * 2]);
            expect(jsRing[i].r).toBe(wasmFlat[i * 2 + 1]);
        }
    });

    it.each([1, 2, 3])('ring(%i) from offset center order matches WASM', (radius) => {
        const cq = 13,
            cr = -51;
        const jsRing = new ChunkId(cq, cr).ring(radius);
        const wasmFlat = hex_ring(cq, cr, radius);

        expect(jsRing.length).toBe(wasmFlat.length / 2);
        for (let i = 0; i < jsRing.length; i++) {
            expect(jsRing[i].q).toBe(wasmFlat[i * 2]);
            expect(jsRing[i].r).toBe(wasmFlat[i * 2 + 1]);
        }
    });
});

describe('ChunkId.toWorldPosition vs WASM hex_flat_to_position', () => {
    it.each([
        [0, 0, 0, 0],
        [1, -1, 0, 0],
        [0, -1, 0, 0],
        [-1, 0, 0, 0],
        [3, -2, 1, 1],
        [0, 0, 5, -3]
    ] as [number, number, number, number][])('chunk (%i,%i) relative to ref (%i,%i)', (q, r, rq, rr) => {
        const js = new ChunkId(q, r).toWorldPosition(new ChunkId(rq, rr));
        // WASM gives absolute positions; compute relative the same way
        const wasmChunk = hex_flat_to_position(q, r, ChunkConst.WORLD_SIZE);
        const wasmRef = hex_flat_to_position(rq, rr, ChunkConst.WORLD_SIZE);
        expect(js.x).toBeCloseTo(wasmChunk[0] - wasmRef[0], 2);
        expect(js.y).toBeCloseTo(wasmChunk[1] - wasmRef[1], 2);
    });
});

describe('ChunkId.fromWorldPosition vs WASM hex_flat_from_position', () => {
    it.each([
        [0, 0, 1, -1],
        [0, 0, 0, -1],
        [0, 0, -1, 1],
        [0, 0, 0, 1],
        [3, -2, 4, -2],
        [3, -2, 2, -3]
    ] as [number, number, number, number][])('ref (%i,%i) → target (%i,%i) round-trips', (rq, rr, tq, tr) => {
        const ref = new ChunkId(rq, rr);
        const target = new ChunkId(tq, tr);
        const pos = target.toWorldPosition(ref);

        // JS inverse
        const jsResult = ChunkId.fromWorldPosition(ref, pos);
        expect(jsResult.equals(target)).toBe(true);

        // WASM inverse (absolute position)
        const wasmRef = hex_flat_to_position(rq, rr, ChunkConst.WORLD_SIZE);
        const absX = pos.x + wasmRef[0];
        const absY = pos.y + wasmRef[1];
        const wasmResult = hex_flat_from_position(absX, absY, ChunkConst.WORLD_SIZE);
        expect(wasmResult[0]).toBe(tq);
        expect(wasmResult[1]).toBe(tr);
    });
});
