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
