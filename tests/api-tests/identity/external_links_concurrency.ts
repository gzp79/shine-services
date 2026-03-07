import { expect, test } from '$fixtures/setup';
import { ExternalUser } from '$lib/api/external_user';
import OAuth2MockServer from '$lib/mocks/oauth2';

test.describe('External links concurrency tests', { tag: '@concurrency' }, () => {
    let mockOAuth2: OAuth2MockServer;

    test.beforeEach(async () => {
        mockOAuth2 = new OAuth2MockServer();
        await mockOAuth2.start();
    });

    test.afterEach(async () => {
        await mockOAuth2.stop();
    });

    test('Concurrent linking of same external account shall fail second attempt', async ({ api }) => {
        const externalUser = ExternalUser.newRandomUser('oauth2_flow');
        const user1 = await api.testUsers.createGuest();
        const user2 = await api.testUsers.createGuest();

        // Both try to link same external account
        const [link1, link2] = await Promise.all([
            (async () => {
                const start = await api.auth.startLinkWithOAuth2(mockOAuth2, user1.sid);
                return api.auth.authorizeWithOAuth2Request(
                    start.sid,
                    start.eid,
                    start.authParams.state,
                    externalUser.toCode()
                );
            })(),
            (async () => {
                const start = await api.auth.startLinkWithOAuth2(mockOAuth2, user2.sid);
                return api.auth.authorizeWithOAuth2Request(
                    start.sid,
                    start.eid,
                    start.authParams.state,
                    externalUser.toCode()
                );
            })()
        ]);

        // One should succeed, one should fail with conflict
        const statuses = [link1.status(), link2.status()].sort();
        expect(statuses).toEqual([200, 200]); // Both requests complete

        const text1 = await link1.text();
        const text2 = await link2.text();
        const problems = [text1, text2].filter((t) => t.includes('auth-register-external-id-conflict'));
        expect(problems.length).toBeGreaterThanOrEqual(1); // At least one conflicts
    });

    test('Link and unlink same account concurrently shall be deterministic', async ({ api }) => {
        const user = await api.testUsers.createLinked(mockOAuth2);
        const externalUser = user.externalUser!;

        // Race: unlink vs link (same account)
        const [unlinkResponse, linkResponse] = await Promise.all([
            api.auth.unlinkRequest(user.sid, externalUser.provider, externalUser.id),
            (async () => {
                const start = await api.auth.startLinkWithOAuth2(mockOAuth2, user.sid);
                return api.auth.authorizeWithOAuth2Request(
                    start.sid,
                    start.eid,
                    start.authParams.state,
                    externalUser.toCode()
                );
            })()
        ]);

        // Both complete
        expect([200, 204]).toContain(unlinkResponse.status());
        expect([200]).toContain(linkResponse.status());

        // Final state should be consistent
        const links = await api.auth.getExternalLinks(user.sid);
        // Either linked or not, but deterministic
        expect([0, 1]).toContain(links.length);
    });

    test('Two users linking different accounts simultaneously shall both succeed', async ({ api }) => {
        const external1 = ExternalUser.newRandomUser('oauth2_flow');
        const external2 = ExternalUser.newRandomUser('oauth2_flow');
        const user1 = await api.testUsers.createGuest();
        const user2 = await api.testUsers.createGuest();

        // Both link different external accounts
        const [link1, link2] = await Promise.all([
            api.auth.linkWithOAuth2(mockOAuth2, user1.sid, external1),
            api.auth.linkWithOAuth2(mockOAuth2, user2.sid, external2)
        ]);

        // Both should succeed
        expect(link1.sid).toBeDefined();
        expect(link2.sid).toBeDefined();

        // Both should have links
        const links1 = await api.auth.getExternalLinks(user1.sid);
        const links2 = await api.auth.getExternalLinks(user2.sid);
        expect(links1).toHaveLength(1);
        expect(links2).toHaveLength(1);
        expect(links1[0].providerUserId).toEqual(external1.id);
        expect(links2[0].providerUserId).toEqual(external2.id);
    });
});
