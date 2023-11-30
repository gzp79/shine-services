import { RequestAPI } from './api';
import { Response } from '../request';

export interface ActiveToken {
    userId: string;
    kind: string;
    tokenFingerprint: string;
    createdAt: Date;
    expireAt: Date;
    isExpired: boolean;
    agent: string;
    country: string | null;
    region: string | null;
    city: string | null;
}

export class TokenAPI {
    constructor(public readonly request: RequestAPI) {}

    async getTokens(sid: string, extraHeaders?: Record<string, string>): Promise<ActiveToken[]> {
        let response = await this.request
            .getTokens(sid)
            .set(extraHeaders ?? {})
            .send();
        expect(response.statusCode).toEqual(200);

        response.body?.tokens?.forEach((t: ActiveToken) => {
            t.createdAt = new Date(t.createdAt);
            t.expireAt = new Date(t.expireAt);
        });

        return response.body?.tokens ?? [];
    }

    async createSAToken(
        sid: string,
        duration: number,
        extraHeaders?: Record<string, string>
    ): Promise<Response> {
        let response = await this.request
            .createSAToken(sid, duration)
            .set(extraHeaders ?? {})
            .send();
        return response;
    }
}
