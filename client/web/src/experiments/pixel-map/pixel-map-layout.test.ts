import { describe, expect, it } from 'vitest';
import { PALETTE_SIZE, cellAtPoint, cellCenter, paletteRgba } from './pixel-map-layout';

describe('paletteRgba', () => {
    it('has an opaque color for every palette index', () => {
        for (let index = 0; index < PALETTE_SIZE; index++) {
            const [, , , alpha] = paletteRgba(index);
            expect(alpha).toBe(255);
        }
    });

    it('keeps the colors distinct, the empty pixel included', () => {
        const colors = new Set<string>();
        for (let index = 0; index < PALETTE_SIZE; index++) colors.add(paletteRgba(index).join(','));

        expect(colors.size).toBe(PALETTE_SIZE);
    });

    it('clamps an index outside the palette to the empty pixel', () => {
        expect(paletteRgba(99)).toEqual(paletteRgba(0));
        expect(paletteRgba(-1)).toEqual(paletteRgba(0));
    });
});

describe('cellCenter', () => {
    it('centers the map on the origin, row 0 at the bottom', () => {
        expect(cellCenter(0, 0, 32, 32)).toEqual([-15.5, -15.5]);
        expect(cellCenter(31, 31, 32, 32)).toEqual([15.5, 15.5]);
        expect(cellCenter(16, 15, 32, 32)).toEqual([0.5, -0.5]);
    });
});

describe('cellAtPoint', () => {
    it('finds the cell under a world point', () => {
        expect(cellAtPoint(-15.9, -15.9, 32, 32)).toEqual({ x: 0, y: 0 });
        expect(cellAtPoint(15.9, 15.9, 32, 32)).toEqual({ x: 31, y: 31 });
        expect(cellAtPoint(0.2, -0.2, 32, 32)).toEqual({ x: 16, y: 15 });
    });

    it('is null off the map', () => {
        expect(cellAtPoint(-16.01, 0, 32, 32)).toBeNull();
        expect(cellAtPoint(16, 0, 32, 32)).toBeNull();
        expect(cellAtPoint(0, 16.5, 32, 32)).toBeNull();
        expect(cellAtPoint(Number.NaN, 0, 32, 32)).toBeNull();
    });

    it('agrees with cellCenter', () => {
        const [x, y] = cellCenter(7, 20, 32, 32);

        expect(cellAtPoint(x, y, 32, 32)).toEqual({ x: 7, y: 20 });
    });
});
