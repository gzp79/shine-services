const BITS_PER_BYTE = 8, BITS_PER_VARINT_BYTE = 7, U8_BYTES = 1, U16_BYTES = 2, U32_BYTES = 4, U64_BYTES = 8, U128_BYTES = 16

const de_zig_zag_signed = (n) => (n >> 1n) ^ (-(n & 0b1n))
const zig_zag = (n_bytes, n) => (n << 1n) ^ (n >> BigInt(n_bytes * BITS_PER_BYTE - 1))
const varint_max = (n_bytes) => Math.floor((n_bytes * BITS_PER_BYTE + (BITS_PER_BYTE - 1)) / BITS_PER_VARINT_BYTE)
const max_of_last_byte = (n_bytes) => (1 << (n_bytes * BITS_PER_BYTE) % 7) - 1
const to_number_if_safe = (n) => Number.MAX_SAFE_INTEGER < ((n < 0n) ? -n : n) ? n : Number(n)
const varint = (n_bytes, n) => { let value = BigInt(n), out = []; for (let i = 0; i < varint_max(n_bytes); i++) { out.push(Number(value & 0xFFn)); if (value < 128n) { return out } out[i] |= 0x80; value >>= 7n } }

class Serializer {
    constructor() { this.bytes = [] }
    finish = () => new Uint8Array(this.bytes)
    push_n = (bytes) => bytes.forEach((byte) => this.bytes.push(byte))
    serialize_bool = (value) => this.serialize_number(U8_BYTES, false, value ? 1 : 0)
    serialize_number = (n_bytes, signed, value) => { if (n_bytes === U8_BYTES) { this.bytes.push(new Uint8Array([value])[0]) } else if (n_bytes === U16_BYTES || n_bytes === U32_BYTES || n_bytes === U64_BYTES || n_bytes === U128_BYTES) { const value_b = BigInt(value), buffer = signed ? varint(n_bytes, zig_zag(n_bytes, value_b)) : varint(n_bytes, value_b); this.push_n(buffer) } else { throw "byte count not supported" } }
    serialize_number_float = (n_bytes, value) => { const b_buffer = new ArrayBuffer(n_bytes), b_view = new DataView(b_buffer); if (n_bytes === U32_BYTES) { b_view.setFloat32(0, value, true) } else if (n_bytes === U64_BYTES) { b_view.setFloat64(0, value, true) } else { throw "byte count not supported" } this.push_n(new Uint8Array(b_buffer)) }
    serialize_string = (str) => { this.push_n(varint(U32_BYTES, str.length)); const bytes = []; for (const c of str) { bytes.push(c.charCodeAt(0)) } this.push_n(bytes) }
    serialize_array = (ser, array, len) => { if (len == undefined) this.push_n(varint(U32_BYTES, array.length)); array.slice(0, len != undefined ? len : array.length).forEach((v) => ser(this, v)) }
    serialize_string_key_map = (ser, obj) => { const entries = Object.entries(obj); this.push_n(varint(U32_BYTES, entries.length)); entries.forEach(([i, v]) => { this.serialize_string(i); ser(this, v) }) }
    serialize_map = (ser, map) => { this.push_n(varint(U32_BYTES, map.size)); map.forEach((v, k) => ser(this, k, v)) }
}

class Deserializer {
    constructor(bytes_in) { this.bytes = Array.from(bytes_in); }
    pop_next = () => { const next = this.bytes.shift(); if (next === undefined) { throw "input buffer too small" } return next }
    pop_n = (n) => { const bytes = Array(); for (let i = 0; i < n; i++) { bytes.push(this.bytes.shift()) } return bytes }
    get_int8 = (signed) => signed ? new Int8Array([this.pop_next()])[0] : this.pop_next();
    try_take = (n_bytes) => { let out = 0n, v_max = varint_max(n_bytes); for (let i = 0; i < v_max; i++) { const val = this.pop_next(), carry = BigInt(val & 0x7F); out |= carry << BigInt(7 * i); if ((val & 0x80) === 0) { if (i === v_max - 1 && val > max_of_last_byte(n_bytes)) { throw "Bad Variant" } else return out } } throw "Bad Variant"; }
    deserialize_bool = () => { const byte = this.pop_next(); return byte === undefined ? undefined : byte > 0 ? true : false }
    deserialize_number = (n_bytes, signed) => { if (n_bytes === U8_BYTES) { return this.get_int8(signed) } else if (n_bytes === U16_BYTES || n_bytes === U32_BYTES || n_bytes === U64_BYTES || n_bytes === U128_BYTES) { const val = this.try_take(n_bytes); return to_number_if_safe(signed ? de_zig_zag_signed(val) : val) } else { throw "byte count not supported" } }
    deserialize_number_float = (n_bytes) => { const b_buffer = new ArrayBuffer(n_bytes), b_view = new DataView(b_buffer); this.pop_n(n_bytes).forEach((b, i) => b_view.setUint8(i, b)); if (n_bytes === U32_BYTES) { return b_view.getFloat32(0, true) } else if (n_bytes === U64_BYTES) { return b_view.getFloat64(0, true) } else { throw "byte count not supported" } }
    deserialize_string = () => { const str = this.pop_n(Number(this.try_take(U32_BYTES))); return String.fromCharCode(...str) }
    deserialize_array = (des, len) => Array.from({length: len === undefined ? Number(this.try_take(U32_BYTES)) : len}, (v, i) => des(this))
    deserialize_string_key_map = (des) => { return [...Array(Number(this.try_take(U32_BYTES)))].reduce((prev) => { prev[this.deserialize_string()] = des(this); return prev }, {}) }
    deserialize_map = (des) => { return [...Array(Number(this.try_take(U32_BYTES)))].reduce((prev) => { const d = des(this); prev.set(d[0], d[1]); return prev }, new Map()) }
    release_bytes = () => { return new Uint8Array(this.bytes); }
}

function check_bounds(v, n_bytes, signed, zero_able) {
    if (!zero_able && v === 0) {
        throw new Error("Value must not be zero")
    }
    const max = BigInt(2 ** (n_bytes * BITS_PER_BYTE)), value_b = BigInt(v);
    if (signed) {
        const bounds = max / 2n;
        if (value_b < -bounds || value_b >= bounds) {
            throw new Error("Value " + value_b + " is out of bounds (" + -bounds + ".." + bounds + ")")
        }
    } else {
        if (value_b >= max || value_b < 0) {
            throw new Error("Value " + value_b + " is out of bounds (0.." + max + ")")
        }
    }

    return true
}

function check_integer_type(v, n_bytes, signed, zero_able) {
    return (
        typeof v === "number" &&
        Number.isInteger(v) ||
        typeof v === "bigint"
    ) && check_bounds(v, n_bytes, signed, zero_able)
}

function is_CHAT_MESSAGE(v) {
    return typeof v === "object" &&
         Array.isArray(v.id) &&
         v.id.every((v) => check_integer_type(v, U8_BYTES, false, true)) &&
         Array.isArray(v.userId) &&
         v.userId.every((v) => check_integer_type(v, U8_BYTES, false, true)) &&
         typeof v.text === "string" &&
         typeof v.edited === "boolean";
}

function is_CHAT_STATE(v) {
    return typeof v === "object" &&
         Array.isArray(v.messages) &&
         v.messages.every((v) => is_CHAT_MESSAGE(v));
}

function is_CHAT_COMMAND(v) {
    return (typeof v === "object" &&
         "tag" in v &&
         "value" in v &&
         (v.tag === "add" &&
         typeof v.value === "object" &&
         typeof v.value.text === "string") ||
         (v.tag === "remove" &&
         typeof v.value === "object" &&
         Array.isArray(v.value.id) &&
         v.value.id.every((v) => check_integer_type(v, U8_BYTES, false, true))) ||
         (v.tag === "replace" &&
         typeof v.value === "object" &&
         Array.isArray(v.value.id) &&
         v.value.id.every((v) => check_integer_type(v, U8_BYTES, false, true)) &&
         typeof v.value.text === "string"))
}

function is_PIXEL_MAP_STATE(v) {
    return typeof v === "object" &&
         Array.isArray(v.pixels) &&
         v.pixels.every((v) => check_integer_type(v, U8_BYTES, false, true));
}

function is_PIXEL_MAP_COMMAND(v) {
    return (typeof v === "object" &&
         "tag" in v &&
         "value" in v &&
         (v.tag === "paint" &&
         typeof v.value === "object" &&
         check_integer_type(v.value.x, U8_BYTES, false, true) &&
         check_integer_type(v.value.y, U8_BYTES, false, true) &&
         check_integer_type(v.value.color, U8_BYTES, false, true)))
}

function serialize_CHAT_MESSAGE(s, v) {
    const lambda_v_id = (s, v) => {
        s.serialize_number(U8_BYTES, false, v)
    };
    s.serialize_array(lambda_v_id, v.id);
    const lambda_v_userId = (s, v) => {
        s.serialize_number(U8_BYTES, false, v)
    };
    s.serialize_array(lambda_v_userId, v.userId);
    s.serialize_string(v.text);
    s.serialize_bool(v.edited);
}

function serialize_CHAT_STATE(s, v) {
    const lambda_v_messages = (s, v) => {
        serialize_CHAT_MESSAGE(s, v)
    };
    s.serialize_array(lambda_v_messages, v.messages);
}

function serialize_CHAT_COMMAND(s, v) {
    switch (v.tag) {
    case "add":
        s.serialize_number(U32_BYTES, false, 0);
        s.serialize_string(v.value.text);
        break;
    case "remove":
        {
            s.serialize_number(U32_BYTES, false, 1);
            const lambda_v_value_id = (s, v) => {
                s.serialize_number(U8_BYTES, false, v)
            };
            s.serialize_array(lambda_v_value_id, v.value.id);
        }
        break;
    case "replace":
        {
            s.serialize_number(U32_BYTES, false, 2);
            const lambda_v_value_id = (s, v) => {
                s.serialize_number(U8_BYTES, false, v)
            };
            s.serialize_array(lambda_v_value_id, v.value.id);
            s.serialize_string(v.value.text);
        }
        break;
    default:
        throw "variant not implemented"
    }
}

function serialize_PIXEL_MAP_STATE(s, v) {
    const lambda_v_pixels = (s, v) => {
        s.serialize_number(U8_BYTES, false, v)
    };
    s.serialize_array(lambda_v_pixels, v.pixels);
}

function serialize_PIXEL_MAP_COMMAND(s, v) {
    switch (v.tag) {
    case "paint":
        {
            s.serialize_number(U32_BYTES, false, 0);
            s.serialize_number(U8_BYTES, false, v.value.x);
            s.serialize_number(U8_BYTES, false, v.value.y);
            s.serialize_number(U8_BYTES, false, v.value.color);
        }
        break;
    default:
        throw "variant not implemented"
    }
}

/**
 * Serialize a value to an array of bytes.
 * @param {string} type - The type of the value to serialize.
 * @param {Object} value - The value to serialize.
 * @return {Uint8Array} The serialized value as an array of bytes.
 */
function serialize(type, value) {
    if (!(typeof type === "string")) {
        throw new Error("type must be a string");
    }
    const s = new Serializer();
    switch (type) {
    case "ChatMessage":
        if (is_CHAT_MESSAGE(value)) {
            serialize_CHAT_MESSAGE(s, value);
        } else {
            throw new Error("Value " + "ChatMessage" + " has wrong format");
        }
        break;
    case "ChatState":
        if (is_CHAT_STATE(value)) {
            serialize_CHAT_STATE(s, value);
        } else {
            throw new Error("Value " + "ChatState" + " has wrong format");
        }
        break;
    case "ChatCommand":
        if (is_CHAT_COMMAND(value)) {
            serialize_CHAT_COMMAND(s, value);
        } else {
            throw new Error("Value " + "ChatCommand" + " has wrong format");
        }
        break;
    case "PixelMapState":
        if (is_PIXEL_MAP_STATE(value)) {
            serialize_PIXEL_MAP_STATE(s, value);
        } else {
            throw new Error("Value " + "PixelMapState" + " has wrong format");
        }
        break;
    case "PixelMapCommand":
        if (is_PIXEL_MAP_COMMAND(value)) {
            serialize_PIXEL_MAP_COMMAND(s, value);
        } else {
            throw new Error("Value " + "PixelMapCommand" + " has wrong format");
        }
        break;
    default:
        throw "type not implemented";
    }
    return s.finish();
}

export {
    serialize
};

function deserialize_CHAT_MESSAGE(d) {
    return {
        id: d.deserialize_array(() => d.deserialize_number(U8_BYTES, false)),
        userId: d.deserialize_array(() => d.deserialize_number(U8_BYTES, false)),
        text: d.deserialize_string(),
        edited: d.deserialize_bool()
    };
}

function deserialize_CHAT_STATE(d) {
    return {
        messages: d.deserialize_array(() => deserialize_CHAT_MESSAGE(d))
    };
}

function deserialize_CHAT_COMMAND(d) {
    switch (d.deserialize_number(U32_BYTES, false)) {
    case 0:
        return {
            tag: "add",
            value: {
                text: d.deserialize_string()
            }
        };
    case 1:
        return {
            tag: "remove",
            value: {
                id: d.deserialize_array(() => d.deserialize_number(U8_BYTES, false))
            }
        };
    case 2:
        return {
            tag: "replace",
            value: {
                id: d.deserialize_array(() => d.deserialize_number(U8_BYTES, false)),
                text: d.deserialize_string()
            }
        };
    default:
        throw "variant not implemented"
    }
}

function deserialize_PIXEL_MAP_STATE(d) {
    return {
        pixels: d.deserialize_array(() => d.deserialize_number(U8_BYTES, false))
    };
}

function deserialize_PIXEL_MAP_COMMAND(d) {
    switch (d.deserialize_number(U32_BYTES, false)) {
    case 0:
        return {
            tag: "paint",
            value: {
                x: d.deserialize_number(U8_BYTES, false),
                y: d.deserialize_number(U8_BYTES, false),
                color: d.deserialize_number(U8_BYTES, false)
            }
        };
    default:
        throw "variant not implemented"
    }
}

/**
 * Deserialize a value from an array of bytes.
 * @param {string} type - The type of the value to deserialize.
 * @param {Uint8Array} bytes - The byte array to deserialize from.
 * @return {Object} The deserialized value and remaining bytes.
 */
function deserialize(type, bytes) {
    if (!(typeof type === "string")) {
        throw "type must be a string";
    }
    const d = new Deserializer(bytes);
    var return_value = undefined;
    switch (type) {
    case "ChatMessage":
        return_value = deserialize_CHAT_MESSAGE(d);
        break;
    case "ChatState":
        return_value = deserialize_CHAT_STATE(d);
        break;
    case "ChatCommand":
        return_value = deserialize_CHAT_COMMAND(d);
        break;
    case "PixelMapState":
        return_value = deserialize_PIXEL_MAP_STATE(d);
        break;
    case "PixelMapCommand":
        return_value = deserialize_PIXEL_MAP_COMMAND(d);
        break;
    default:
        throw "type not implemented";
    }
    return { value: return_value, bytes: d.release_bytes() };
}

export {
    deserialize
};
