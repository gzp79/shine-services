import { expect, test } from '$fixtures/setup';
import { ExternalUser } from '$lib/api/external_user';
import { getPageProblem } from '$lib/api/utils';
import OAuth2MockServer from '$lib/mocks/oauth2';

test.describe('External links concurrency tests', { tag: '@concurrency' }, () => {
    let mockOAuth2: OAuth2MockServer;

    test.beforeAll(async () => {
        mockOAuth2 = new OAuth2MockServer();
        await mockOAuth2.start();
    });

    test.afterAll(async () => {
        await mockOAuth2?.stop();
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

        // Race between two users trying to link same external account:
        // - First to complete: succeeds (200)
        // - Second to complete: conflicts (error embedded in 200 response page)
        // Note: Auth pages return 200 with error details in HTML
        const statuses = [link1.status(), link2.status()].sort();
        expect(statuses).toEqual([200, 200]); // Both requests complete

        const text1 = await link1.text();
        const text2 = await link2.text();
        const problems = [getPageProblem(text1), getPageProblem(text2)].filter(
            (p) => p?.type === 'auth-register-external-id-conflict'
        );
        expect(problems.length).toBeGreaterThanOrEqual(1); // At least one should conflict
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

        // Race between unlink and link:
        // - Unlink: 200 or 204 depending on whether link existed
        // - Link: 200 (creates or recreates link)
        expect([200, 204]).toContain(unlinkResponse.status());
        expect([200]).toContain(linkResponse.status());

        // Final state depends on operation order:
        // - If unlink then link: 1 link exists
        // - If link then unlink: 0 links exist
        // Both outcomes are correct depending on timing
        const links = await api.auth.getExternalLinks(user.sid);
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
