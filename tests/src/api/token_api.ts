import { expect } from '$fixtures/setup';
import { DateStringSchema, OptionalSchema } from '$lib/schema_utils';
import { joinURL } from '$lib/utils';
import { z } from 'zod';
import { ApiRequest } from './api';

const TokenKindSchema = z.enum(['singleAccess', 'persistent', 'access', 'emailAccess']);
export type TokenKind = z.infer<typeof TokenKindSchema>;

const ActiveTokenSchema = z.object({
    userId: z.string(),
    tokenHash: z.string(),
    kind: TokenKindSchema,
    createdAt: DateStringSchema,
    expireAt: DateStringSchema,
    isExpired: z.boolean(),
    agent: z.string(),
    country: OptionalSchema(z.string()),
    region: OptionalSchema(z.string()),
    city: OptionalSchema(z.string())
});
export type ActiveToken = z.infer<typeof ActiveTokenSchema>;

const ActiveTokensSchema = z.object({
    tokens: z.array(ActiveTokenSchema)
});
export type ActiveTokens = z.infer<typeof ActiveTokensSchema>;

const TokenSchema = z.object({
    kind: z.string(),
    token: z.string(),
    tokenHash: z.string(),
    tokenType: z.string(),
    expireAt: DateStringSchema
});
export type Token = z.infer<typeof TokenSchema>;

export const CreateTokenRequestSchema = z.object({
    kind: TokenKindSchema,
    timeToLive: z.number(),
    bindToSite: z.boolean()
});
export type CreateTokenRequest = z.infer<typeof CreateTokenRequestSchema>;

export class TokenAPI {
    constructor(public readonly serviceUrl: string) {}

    urlFor(path: string) {
        return joinURL(new URL(this.serviceUrl), path);
    }

    getTokensRequest(sid: string | null): ApiRequest {
        const cs = sid && { sid };

        return ApiRequest.get(this.urlFor('api/auth/user/tokens')).withCookies({ ...cs });
    }

    async getTokens(sid: string, extraHeaders?: Record<string, string>): Promise<ActiveToken[]> {
        const response = await this.getTokensRequest(sid).withHeaders(extraHeaders ?? {});
        expect(response).toHaveStatus(200);

        return (await response.parse(ActiveTokensSchema)).tokens;
    }

    getTokenRequest(sid: string | null, tokenId: string): ApiRequest {
        const cs = sid && { sid };

        return ApiRequest.get(this.urlFor(`api/auth/user/tokens/${tokenId}`)).withCookies({ ...cs });
    }

    async getToken(sid: string | null, tokenId: string): Promise<ActiveToken | undefined> {
        const response = await this.getTokenRequest(sid, tokenId);
        if (response.status() === 404) {
            return undefined;
        }
        expect(response).toHaveStatus(200);
        return await response.parse(ActiveTokenSchema);
    }

    revokeTokenRequest(sid: string | null, tokenId: string): ApiRequest {
        const cs = sid && { sid };

        return ApiRequest.delete(this.urlFor(`api/auth/user/tokens/${tokenId}`)).withCookies({ ...cs });
    }

    async revokeToken(sid: string | null, tokenId: string): Promise<void> {
        const response = await this.revokeTokenRequest(sid, tokenId);
        expect(response).toHaveStatus(200);
    }

    createTokenRequest(
        sid: string | null,
        kind: TokenKind,
        duration: number,
        bindToSite: boolean
    ): ApiRequest<CreateTokenRequest> {
        const cs = sid && { sid };

        return ApiRequest.post<CreateTokenRequest>(this.urlFor('api/auth/user/tokens'), {
            kind,
            timeToLive: duration,
            bindToSite: bindToSite
        }).withCookies({ ...cs });
    }

    async createSAToken(
        sid: string,
        duration: number,
        bindToSite: boolean,
        extraHeaders?: Record<string, string>
    ): Promise<Token> {
        const response = await this.createTokenRequest(sid, 'singleAccess', duration, bindToSite).withHeaders(
            extraHeaders ?? {}
        );
        expect(response).toHaveStatus(200);

        const token = await response.parse(TokenSchema);
        expect(token.kind).toEqual('singleAccess');

        return token;
    }

    async createPersistentToken(
        sid: string,
        duration: number,
        bindToSite: boolean,
        extraHeaders?: Record<string, string>
    ): Promise<Token> {
        const response = await this.createTokenRequest(sid, 'persistent', duration, bindToSite).withHeaders(
            extraHeaders ?? {}
        );
        expect(response).toHaveStatus(200);

        const token = await response.parse(TokenSchema);
        expect(token.tokenType).toEqual('Bearer');
        expect(token.kind).toEqual('persistent');

        return token;
    }
}
