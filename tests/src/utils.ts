import crypto from 'crypto';

export function delay(ms: number): Promise<void> {
    return new Promise((resolve) => setTimeout(resolve, ms));
}

export function joinURL(baseUrl: URL | string, path: string): string {
    let base = baseUrl.toString();
    if (!base.endsWith('/')) {
        base += '/';
    }
    if (path.startsWith('/')) {
        path = path.substring(1);
    }
    return base + path;
}

export function convertKeysToLowerCase<T>(obj: Record<string, T>): Record<string, T> {
    return Object.keys(obj).reduce(
        (acc, key) => {
            acc[key.toLowerCase()] = obj[key];
            return acc;
        },
        {} as Record<string, T>
    );
}

export function removeUndefinedValues<T>(obj: Record<string, T | undefined>): Record<string, T> {
    return Object.keys(obj).reduce(
        (acc, key) => {
            if (obj[key] !== undefined) {
                acc[key] = obj[key];
            }
            return acc;
        },
        {} as Record<string, T>
    );
}

/* eslint-disable @typescript-eslint/no-explicit-any */
export function parseSignedCookie(value: string): any {
    const json = decodeURIComponent(value);
    const payload = json.substring(44);
    return JSON.parse(payload);
}
/* eslint-enable @typescript-eslint/no-explicit-any */

export function getSHA256Hash(text: string): string {
    const hash = crypto.createHash('sha256');
    hash.update(text);
    return hash.digest('hex');
}
