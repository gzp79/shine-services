import { expect } from '$fixtures/setup';
import { DateStringSchema, OptionalSchema } from '$lib/schema_utils';
import { joinURL } from '$lib/utils';
import { z } from 'zod';
import { ApiClient, ApiRequest } from './api';

const ActiveSessionSchema = z.object({
    userId: z.string(),
    fingerprint: z.string(),
    createdAt: DateStringSchema,
    agent: z.string(),
    country: OptionalSchema(z.string()),
    region: OptionalSchema(z.string()),
    city: OptionalSchema(z.string())
});
export type ActiveSession = z.infer<typeof ActiveSessionSchema>;

const ActiveSessionsSchema = z.object({
    sessions: z.array(ActiveSessionSchema)
});
export type ActiveSessions = z.infer<typeof ActiveSessionsSchema>;

export class SessionAPI {
    private readonly client: ApiClient;

    constructor(
        public readonly serviceUrl: string,
        public readonly enableRequestLogging: boolean
    ) {
        this.client = new ApiClient(enableRequestLogging);
    }

    urlFor(path: string) {
        return joinURL(new URL(this.serviceUrl), path);
    }

    getSessionsRequest(sid: string | null): ApiRequest {
        const cs = sid && { sid };

        return this.client.get(this.urlFor('api/auth/user/sessions')).withCookies({ ...cs });
    }

    async getSessions(sid: string, extraHeaders?: Record<string, string>): Promise<ActiveSession[]> {
        const response = await this.getSessionsRequest(sid).withHeaders(extraHeaders ?? {});
        expect(response).toHaveStatus(200);

        return (await response.parse(ActiveSessionsSchema)).sessions;
    }
}
