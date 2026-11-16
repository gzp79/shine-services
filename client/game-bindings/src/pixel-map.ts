import type {
    PixelMapCommand as PixelMapCommandType,
    PixelMapState as PixelMapStateType
} from '../generated/postcard-bindings';
import { PIXEL_MAP_COLORS, PIXEL_MAP_HEIGHT, PIXEL_MAP_WIDTH } from '../generated/snapshots';
import { wireCodec } from './codec';

export { PIXEL_MAP_COLORS, PIXEL_MAP_HEIGHT, PIXEL_MAP_WIDTH };

export type PixelPaint = { x: number; y: number; color: number };

export type PixelMapState = PixelMapStateType;
export const PixelMapState = wireCodec('PixelMapState');

export type PixelMapCommand = PixelMapCommandType;

export const PixelMapCommand = {
    ...wireCodec('PixelMapCommand'),
    paint: (x: number, y: number, color: number): PixelMapCommand => ({ tag: 'paint', value: { x, y, color } }),
    paintMany: (paints: readonly PixelPaint[]): PixelMapCommand[] =>
        paints.map(({ x, y, color }) => PixelMapCommand.paint(x, y, color))
};
