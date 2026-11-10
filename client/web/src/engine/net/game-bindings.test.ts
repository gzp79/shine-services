import {
    CHAT_MAX_TEXT_BYTES,
    ChatCommand,
    ChatState,
    PIXEL_MAP_HEIGHT,
    PIXEL_MAP_WIDTH,
    PixelMapCommand,
    PixelMapState,
    fromBase64,
    toBase64,
    uuidFromBytes,
    uuidToBytes
} from '#game-bindings';
import { describe, expect, it } from 'vitest';

describe('game bindings', () => {
    it('round-trips base64', () => {
        const bytes = Uint8Array.from({ length: 256 }, (_, i) => i);
        expect(fromBase64(toBase64(bytes))).toEqual(bytes);
    });

    it('round-trips uuids', () => {
        const uuid = '00112233-4455-6677-8899-aabbccddeeff';
        expect(uuidFromBytes(uuidToBytes(uuid))).toBe(uuid);
        expect(() => uuidToBytes('nope')).toThrow();
    });

    it('encodes a paint as variant 0 followed by x, y, color', () => {
        expect(PixelMapCommand.encode(PixelMapCommand.paint(1, 2, 3))).toBe('AAECAw==');
        expect(PixelMapCommand.parse('AAECAw==')).toEqual({ tag: 'paint', value: { x: 1, y: 2, color: 3 } });
    });

    it('builds many paints', () => {
        const paints = Array.from({ length: 1000 }, (_, n) => ({
            x: n % PIXEL_MAP_WIDTH,
            y: n % PIXEL_MAP_HEIGHT,
            color: 1
        }));
        const commands = PixelMapCommand.paintMany(paints).map(PixelMapCommand.encode);
        expect(commands).toHaveLength(1000);
        expect(PixelMapCommand.parse(commands[37])).toEqual({ tag: 'paint', value: paints[37] });
    });

    it('round-trips a pixel map state', () => {
        const pixels = Array.from({ length: PIXEL_MAP_WIDTH * PIXEL_MAP_HEIGHT }, (_, n) => n % 16);
        expect(PixelMapState.parse(PixelMapState.encode({ pixels }))).toEqual({ pixels });
    });

    it('round-trips chat commands', () => {
        const id = '00112233-4455-6677-8899-aabbccddeeff';
        for (const command of [ChatCommand.add('hello'), ChatCommand.remove(id), ChatCommand.replace(id, 'fixed')]) {
            expect(ChatCommand.parse(ChatCommand.encode(command))).toEqual(command);
        }
    });

    it('round-trips a chat state', () => {
        const state = {
            messages: [
                {
                    id: uuidToBytes('00000000-0000-0000-0000-000000000001'),
                    userId: uuidToBytes('00000000-0000-0000-0000-0000000003e9'),
                    text: 'hi',
                    edited: true
                }
            ]
        };
        expect(ChatState.parse(ChatState.encode(state))).toEqual(state);
    });

    it('exposes the limits of the states', () => {
        expect(CHAT_MAX_TEXT_BYTES).toBeGreaterThan(0);
    });
});
