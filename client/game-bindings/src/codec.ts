import { type Result, type Type, deserialize, serialize } from '../generated/postcard-bindings';

// Wire encodings shared by the stream helpers. Commands and snapshot data travel as base64 of their postcard bytes.

export function toBase64(bytes: Uint8Array): string {
    let binary = '';
    for (let i = 0; i < bytes.length; i++) binary += String.fromCharCode(bytes[i]);
    return btoa(binary);
}

export function fromBase64(text: string): Uint8Array {
    const binary = atob(text);
    const bytes = new Uint8Array(binary.length);
    for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
    return bytes;
}

// The base64 encode/parse of a generated type.
export const wireCodec = <T extends Type>(type: T) => ({
    encode: (value: Result<T>['value']): string => toBase64(serialize(type, value)),
    parse: (base64: string): Result<T>['value'] => deserialize(type, fromBase64(base64)).value
});

// The generated types hold ids as plain byte lists.
export function uuidToBytes(uuid: string): number[] {
    const hex = uuid.split('-').join('');
    if (!/^[0-9a-fA-F]{32}$/.test(hex)) throw new Error(`invalid uuid: ${uuid}`);
    return Array.from({ length: 16 }, (_, i) => parseInt(hex.slice(i * 2, i * 2 + 2), 16));
}

export function uuidFromBytes(bytes: ArrayLike<number>): string {
    if (bytes.length !== 16) throw new Error(`a uuid has 16 bytes, got ${bytes.length}`);
    const hex = Array.from(bytes, (byte) => byte.toString(16).padStart(2, '0')).join('');
    return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
}
