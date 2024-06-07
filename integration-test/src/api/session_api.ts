import { RequestAPI } from './api';

export interface ActiveSession {
    userId: string;
    createdAt: Date;
    agent: string;
    country: string | null;
    region: string | null;
    city: string | null;
}

export class SessionAPI {
    constructor(public readonly request: RequestAPI) {}

    async getSessions(sid: string, extraHeaders?: Record<string, string>): Promise<ActiveSession[]> {
        let response = await this.request.getSessions(sid).set(extraHeaders ?? {});
        expect(response).toHaveStatus(200);

        response.body?.sessions?.forEach((s: ActiveSession) => {
            s.createdAt = new Date(s.createdAt);
        });
        return response.body?.sessions ?? [];
    }
}
