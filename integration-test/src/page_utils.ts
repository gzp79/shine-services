export function getPageRedirectUrl(page: string): string | undefined {
    const regexp = /.*<meta http-equiv[^>]*url='([^']*)'[^>]*>.*/;
    const match = regexp.exec(page) ?? [];
    return match[1];
}
