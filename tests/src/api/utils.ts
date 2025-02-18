import debug from 'debug';
import { decode } from 'html-entities';
import { Problem, ProblemSchema } from './api';

const log = debug('test:page');

export function getPageRedirectUrl(page: string): string {
    const regexp = /.*<meta\s+http-equiv[^>]*url='([^']*)'[^>]*>.*/;
    const match = regexp.exec(page) ?? [];
    const url = match[1] ?? '';
    log(`Redirect: ${url}`);
    return url;
}

export function getPageProblem(page: string): Problem | null {
    const regexp = /<pre>([\s\S]*?)<\/pre>/;
    const match = regexp.exec(page) ?? [];
    const part = match[1];
    if (part === undefined) {
        return null;
    }
    const json = decode(part);
    log(`Problem: ${json}`);
    return ProblemSchema.parse(JSON.parse(json));
}
