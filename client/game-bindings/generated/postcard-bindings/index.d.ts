declare type u8 = number
declare type u16 = number
declare type u32 = number
declare type u64 = bigint
declare type u128 = bigint
declare type usize = bigint
declare type i8 = number
declare type i16 = number
declare type i32 = number
declare type i64 = bigint
declare type i128 = bigint
declare type isize = bigint
declare type NonZeroU8 = number
declare type NonZeroU16 = number
declare type NonZeroU32 = number
declare type NonZeroU64 = bigint
declare type NonZeroU128 = bigint
declare type NonZeroUsize = bigint
declare type NonZeroI8 = number
declare type NonZeroI16 = number
declare type NonZeroI32 = number
declare type NonZeroI64 = bigint
declare type NonZeroI128 = bigint
declare type NonZeroIsize = bigint
declare type f32 = number
declare type f64 = number

declare type ArrayLengthMutationKeys = "splice" | "push" | "pop" | "shift" | "unshift"
declare type FixedLengthArray<T, L extends number, TObj = [T, ...Array<T>]> =
    Pick<TObj, Exclude<keyof TObj, ArrayLengthMutationKeys>>
    & {
        readonly length: L
        [ I : number ] : T
        [Symbol.iterator]: () => IterableIterator<T>
    }

export type ChatMessage = { id: u8[], userId: u8[], text: string, edited: boolean }
export type ChatState = { messages: ChatMessage[] }
export type ChatCommand = { tag: "add", value: { text: string } } | { tag: "remove", value: { id: u8[] } } | { tag: "replace", value: { id: u8[], text: string } }
export type PixelMapState = { pixels: u8[] }
export type PixelMapCommand = { tag: "paint", value: { x: u8, y: u8, color: u8 } }

export type Type = "ChatMessage" | "ChatState" | "ChatCommand" | "PixelMapState" | "PixelMapCommand"
declare type ValueType<T extends Type> = T extends "ChatMessage" ? ChatMessage : T extends "ChatState" ? ChatState : T extends "ChatCommand" ? ChatCommand : T extends "PixelMapState" ? PixelMapState : T extends "PixelMapCommand" ? PixelMapCommand : void

export function serialize<T extends Type>(type: T, value: ValueType<T>): Uint8Array

export interface Result<T extends Type> {
    value: ValueType<T>;
    bytes: Uint8Array;
}

export function deserialize<T extends Type>(type: T, bytes: Uint8Array): Result<T>
