/* eslint-disable @typescript-eslint/no-explicit-any */
export function parseSignedCookie(value: string): any {
    const json = decodeURIComponent(value);
    const payload = json.substring(44);
    return JSON.parse(payload);
}
/* eslint-enable @typescript-eslint/no-explicit-any */
