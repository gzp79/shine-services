import { RequestAPI } from './api';

export interface ExternalLink {
    userId: string;
    provider: string;
    providerUserId: string;
    linkedAt: Date;
    name: string | null;
    email: string | null;
}

export class ExternalLinkAPI {
    constructor(public readonly request: RequestAPI) {}

    async getExternalLinks(sid: string, extraHeaders?: Record<string, string>): Promise<ExternalLink[]> {
        let response = await this.request.getExternalLinks(sid).set(extraHeaders ?? {});

        expect(response.statusCode).toEqual(200);

        response.body?.links?.forEach((l: ExternalLink) => {
            l.linkedAt = new Date(l.linkedAt);
        });

        return response.body?.links ?? [];
    }

    async tryUnlink(
        sid: string,
        provider: string,
        providerUserId: string,
        extraHeaders?: Record<string, string>
    ): Promise<boolean> {
        let response = await this.request.unlink(sid, provider, providerUserId).set(extraHeaders ?? {});
        if (response.statusCode == 404) {
            return false;
        }

        expect(response.statusCode).toEqual(200);
        return true;
    }
}
