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

export function createUrl(base: URL | string, params: Record<string, undefined | null | string | number>): string {
    const url = new URL(base);
    Object.entries(params).forEach(([key, value]) => {
        if (value === undefined || value === null) {
            return;
        }
        url.searchParams.append(key, String(value));
    });
    return url.toString();
}
