import { createUrlQueryString, generateRandomString } from '$lib/string_utils';
import { randomUUID } from 'crypto';

export class ExternalUser {
    public readonly id: string;
    public readonly name: string;
    public readonly email: string;

    constructor(id: string, name: string, email: string) {
        this.id = id;
        this.name = name;
        this.email = email;
    }

    static newRandomUser(): ExternalUser {
        const name = 'Random_' + generateRandomString(5);
        return new ExternalUser(randomUUID(), name, name + '@example.com');
    }

    toCode(params?: any): string {
        return createUrlQueryString({
            id: this.id,
            name: this.name,
            email: this.email,
            ...params
        });
    }
}
