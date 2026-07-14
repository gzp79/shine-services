import { expect, test } from '$fixtures/setup';
import { SessionMint } from '$lib/mocks/session_mint';
import { parseSignedCookie } from '$lib/utils';
import { randomUUID } from 'node:crypto';

test.describe('Session mint', () => {
    test('mint add/update/teardown shall work for isolated user id', async () => {
        const mint = await SessionMint.fromServerConfig();
        const userId = randomUUID();

        const created = await mint.addUser({
            userId,
            userAgent: 'mint-test-agent-1'
        });

        const createdCookiePayload = parseSignedCookie(created.sessionCookie);
        expect(createdCookiePayload.u).toEqual(userId);
        expect(createdCookiePayload.key).toEqual(created.sessionKeyHex);
        expect(createdCookiePayload.fp).toEqual(created.fingerprint);

        const updated = await mint.updateUser(created, {
            userAgent: 'mint-test-agent-2'
        });

        const updatedCookiePayload = parseSignedCookie(updated.sessionCookie);
        expect(updatedCookiePayload.u).toEqual(userId);
        expect(updatedCookiePayload.key).toEqual(created.sessionKeyHex);
        expect(updatedCookiePayload.fp).toEqual(updated.fingerprint);
        expect(updated.fingerprint).not.toEqual(created.fingerprint);

        await mint.teardownCreatedSessions();
    });
});
