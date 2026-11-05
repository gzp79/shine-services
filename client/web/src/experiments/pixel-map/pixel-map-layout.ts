// Where the cells of the pixel map are in the world and what color they show. The map is centered on
// the origin in the XY plane; row 0 is at the bottom.

export const PALETTE_SIZE = 16;

// Index 0 is the empty pixel.
const PALETTE: readonly (readonly [number, number, number])[] = [
    [34, 34, 51],
    [255, 255, 255],
    [228, 59, 68],
    [247, 118, 34],
    [254, 231, 97],
    [99, 199, 77],
    [24, 140, 90],
    [44, 232, 245],
    [0, 149, 233],
    [54, 84, 180],
    [104, 56, 108],
    [181, 80, 136],
    [246, 117, 122],
    [139, 109, 156],
    [139, 155, 180],
    [38, 43, 68]
];

export function paletteRgba(index: number): [number, number, number, number] {
    const [r, g, b] = PALETTE[index] ?? PALETTE[0];
    return [r, g, b, 255];
}

export function cellCenter(x: number, y: number, width: number, height: number): [number, number] {
    return [x - (width - 1) / 2, y - (height - 1) / 2];
}

export function cellAtPoint(
    worldX: number,
    worldY: number,
    width: number,
    height: number
): { x: number; y: number } | null {
    const x = Math.floor(worldX + width / 2);
    const y = Math.floor(worldY + height / 2);
    if (!(x >= 0 && x < width && y >= 0 && y < height)) {
        return null;
    }
    return { x, y };
}
