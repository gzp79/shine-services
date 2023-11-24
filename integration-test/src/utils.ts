import crypto from 'crypto';

export function delay(ms: number): Promise<void> {
    return new Promise((resolve) => setTimeout(resolve, ms));
}

export function joinURL(baseUrl: URL, path: string): string {
    let base = baseUrl.toString();
    if (!base.endsWith('/')) {
        base += '/';
    }
    if (path.startsWith('/')) {
        path = path.substring(1);
    }
    return base + path;
}

export function parseSignedCookie(value: string): any {
    const json = decodeURIComponent(value);
    return JSON.parse(json.substring(44));
};

export function getSHA256Hash(text: string): string {
    const hash = crypto.createHash('sha256');
    hash.update(text);
    return hash.digest('hex');
  }
