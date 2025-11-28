import { createUrlQueryString, generateRandomString } from '$lib/string_utils';
import { randomUUID } from 'crypto';

export type ExternalUserProvider = 'oauth2_flow' | 'openid_flow';

export class ExternalUser {
    public readonly provider: ExternalUserProvider;
    public readonly id: string;
    public readonly name: string;
    public readonly email: string | undefined;

    constructor(provider: ExternalUserProvider, id: string, name: string, email: string | undefined) {
        this.provider = provider;
        this.id = id;
        this.name = name;
        this.email = email;
    }

    static newRandomUser(provider: ExternalUserProvider): ExternalUser {
        const name = 'Random_' + generateRandomString(5);
        return new ExternalUser(provider, randomUUID(), name, name + '@example.com');
    }

    toCode(params?: Record<string, string>): string {
        return createUrlQueryString({
            id: this.id,
            name: this.name,
            ...(this.email ? { email: this.email } : {}),
            ...params
        });
    }
}
