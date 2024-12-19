import { Cookie } from 'tough-cookie';
import { Response } from '$lib/request';

export function getCookies(response?: Response): Record<string, Cookie> {
    return (response?.get('Set-Cookie') ?? [])
        .map((cookieStr: string) => Cookie.parse(cookieStr)!)
        .reduce((cookies: Record<string, Cookie>, cookie: Cookie) => {
            cookies[cookie.key] = cookie;
            return cookies;
        }, {});
}

export function getPageRedirectUrl(page: string): string | undefined {
    const regexp = /.*<meta http-equiv[^>]*url='([^']*)'[^>]*>.*/;
    const match = regexp.exec(page) ?? [];
    return match[1];
}
