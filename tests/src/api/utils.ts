import debug from 'debug';
import { decode } from 'html-entities';
import { ParsedMail } from 'mailparser';
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

export function getEmailLink(mail: ParsedMail): string {
    const html = mail.textAsHtml ?? '';
    const regexp = /<a\s+href="([^"]*)"[^>]*>.*<\/a>/;
    const match = regexp.exec(html) ?? [];
    const url = match[1] ?? '';
    log(`Email link: ${url}`);
    return url;
}

export function getEmailLinkToken(mail: ParsedMail): string | null {
    const authUrl = getEmailLink(mail);
    if (!authUrl) {
        return null;
    }
    const authParams = new URL(authUrl).searchParams;
    const confirmUrl = authParams.get('redirectUrl');
    if (!confirmUrl) {
        return null;
    }
    
    const token = new URL(confirmUrl).searchParams.get('token');
    return token;
}
