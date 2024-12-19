import { Response } from '../request';
import { RequestAPI } from './api';

export interface ActiveToken {
    userId: string;
    kind: string;
    tokenHash: string;
    createdAt: Date;
    expireAt: Date;
    isExpired: boolean;
    agent: string;
    country: string | null;
    region: string | null;
    city: string | null;
}

export interface Token {
    kind: string;
    token: string;
    tokenHash: string;
    tokenType: string;
    expireAt: Date;
}

export class TokenAPI {
    constructor(public readonly request: RequestAPI) {}

    async getTokens(sid: string, extraHeaders?: Record<string, string>): Promise<ActiveToken[]> {
        let response = await this.request.getTokens(sid).set(extraHeaders ?? {});
        expect(response).toHaveStatus(200);

        response.body?.tokens?.forEach((t: ActiveToken) => {
            t.createdAt = new Date(t.createdAt);
            t.expireAt = new Date(t.expireAt);
        });

        return response.body?.tokens ?? [];
    }

    async createSAToken(
        sid: string,
        duration: number,
        bindToSite: boolean,
        extraHeaders?: Record<string, string>
    ): Promise<Token> {
        let response = await this.request
            .createToken(sid, 'singleAccess', duration, bindToSite)
            .set(extraHeaders ?? {});

        expect(response).toHaveStatus(200);
        expect(response.body.kind).toEqual('singleAccess');

        response.body.expireAt = new Date(response.body.expireAt as string);
        return response.body as Token;
    }

    async createPersistentToken(
        sid: string,
        duration: number,
        bindToSite: boolean,
        extraHeaders?: Record<string, string>
    ): Promise<Token> {
        let response = await this.request
            .createToken(sid, 'persistent', duration, bindToSite)
            .set(extraHeaders ?? {});

        expect(response).toHaveStatus(200);
        expect(response.body.tokenType).toEqual('Bearer');
        expect(response.body.kind).toEqual('persistent');

        response.body.expireAt = new Date(response.body.expireAt as string);
        return response.body as Token;
    }
}
