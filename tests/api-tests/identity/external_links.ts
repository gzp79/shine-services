import { expect, test } from '$fixtures/setup';
import { LinkedIdentity } from '$lib/api/auth_api';
import { ExternalUser } from '$lib/api/external_user';
import OAuth2MockServer from '$lib/mocks/oauth2';
import OpenIdMockServer from '$lib/mocks/openid';

test.describe('External links', () => {
    const now = new Date().getTime();
    const linkRange = [new Date(now - 60 * 1000), new Date(now + 60 * 1000)];

    let mockOAuth2: OAuth2MockServer;
    let mockOpenId: OpenIdMockServer;

    test.beforeEach(async () => {
        mockOAuth2 = new OAuth2MockServer();
        await mockOAuth2.start();

        mockOpenId = new OpenIdMockServer();
        await mockOpenId.start();
    });

    test.afterEach(async () => {
        await mockOAuth2.stop();
        mockOAuth2 = undefined!;
        await mockOpenId.stop();
        mockOpenId = undefined!;
    });

    test('Sign up as guest shall not be linked', async ({ api }) => {
        const user = await api.testUsers.createGuest();
        expect(await api.auth.getExternalLinks(user.sid)).toBeEmpty();
    });

    test('Sign up with OAuth2 shall create a link and delete link shall work', async ({ api }) => {
        const user = await api.testUsers.createLinked(mockOAuth2);
        const links = await api.auth.getExternalLinks(user.sid);

        expect(links).toHaveLength(1);
        const link = links[0];
        expect(link.provider).toEqual('oauth2_flow');
        expect(link.userId).toEqual(user.userId);
        expect(link.email).toEqual(user.externalUser?.email);
        expect(link.name).toEqual(user.externalUser?.name);
        expect(link.linkedAt).toBeAfter(linkRange[0]);
        expect(link.linkedAt).toBeBefore(linkRange[1]);

        expect(await api.auth.tryUnlink(user.sid, link.provider, link.providerUserId)).toBeTruthy();
        expect(await api.auth.getExternalLinks(user.sid)).toBeEmpty();
        expect(await api.auth.tryUnlink(user.sid, link.provider, link.providerUserId)).toBeFalsy();
    });

    test('Sign up with OpenId shall create a link and delete link shall work', async ({ api }) => {
        const user = await api.testUsers.createLinked(mockOpenId);
        const links = await api.auth.getExternalLinks(user.sid);

        expect(links).toHaveLength(1);
        const link = links[0];
        expect(link.provider).toEqual('openid_flow');
        expect(link.userId).toEqual(user.userId);
        expect(link.email).toEqual(user.externalUser?.email);
        expect(link.name).toEqual(user.externalUser?.name);
        expect(link.linkedAt).toBeAfter(linkRange[0]);
        expect(link.linkedAt).toBeBefore(linkRange[1]);

        expect(await api.auth.tryUnlink(user.sid, link.provider, link.providerUserId)).toBeTruthy();
        expect(await api.auth.getExternalLinks(user.sid)).toBeEmpty();
        expect(await api.auth.tryUnlink(user.sid, link.provider, link.providerUserId)).toBeFalsy();
    });

    test('Link to multiple provider and delete links shall work', async ({ api }) => {
        const user = await api.testUsers.createGuest();
        expect(await api.auth.getExternalLinks(user.sid)).toBeEmpty();

        const testLink = (links: LinkedIdentity[], external: ExternalUser, provider: string) => {
            const link = links.find((l) => l.providerUserId === external.id);
            expect(link).toBeDefined();
            expect(link!.provider).toEqual(provider);
            expect(link!.userId).toEqual(user.userId);
            expect(link!.email).toEqual(external.email);
            expect(link!.name).toEqual(external.name);
            expect(link!.linkedAt).toBeAfter(linkRange[0]);
            expect(link!.linkedAt).toBeBefore(linkRange[1]);
        };

        //link with oauth2
        const l1 = ExternalUser.newRandomUser();
        await api.auth.linkWithOAuth2(mockOAuth2, user.sid, l1);
        let links = await api.auth.getExternalLinks(user.sid);
        expect(links).toHaveLength(1);
        testLink(links, l1, 'oauth2_flow');

        // link with openid
        const l2 = ExternalUser.newRandomUser();
        await api.auth.linkWithOpenId(mockOpenId, user.sid, l2);
        links = await api.auth.getExternalLinks(user.sid);
        expect(links).toHaveLength(2);
        testLink(links, l1, 'oauth2_flow');
        testLink(links, l2, 'openid_flow');

        const l3 = ExternalUser.newRandomUser();
        await api.auth.linkWithOAuth2(mockOAuth2, user.sid, l3);
        links = await api.auth.getExternalLinks(user.sid);
        expect(links).toHaveLength(3);
        testLink(links, l1, 'oauth2_flow');
        testLink(links, l2, 'openid_flow');
        testLink(links, l3, 'oauth2_flow');

        const l4 = ExternalUser.newRandomUser();
        await api.auth.linkWithOpenId(mockOpenId, user.sid, l4);
        links = await api.auth.getExternalLinks(user.sid);
        expect(links).toHaveLength(4);
        testLink(links, l1, 'oauth2_flow');
        testLink(links, l2, 'openid_flow');
        testLink(links, l3, 'oauth2_flow');
        testLink(links, l4, 'openid_flow');

        //unlink the first link
        expect(await api.auth.tryUnlink(user.sid, 'oauth2_flow', l1.id)).toBeTruthy();
        links = await api.auth.getExternalLinks(user.sid);
        expect(links).toHaveLength(3);
        testLink(links, l2, 'openid_flow');
        testLink(links, l3, 'oauth2_flow');
        testLink(links, l4, 'openid_flow');
        expect(await api.auth.tryUnlink(user.sid, 'oauth2_flow', l1.id)).toBeFalsy();

        //unlink the last link
        expect(await api.auth.tryUnlink(user.sid, 'openid_flow', l4.id)).toBeTruthy();
        links = await api.auth.getExternalLinks(user.sid);
        expect(links).toHaveLength(2);
        testLink(links, l2, 'openid_flow');
        testLink(links, l3, 'oauth2_flow');
        expect(await api.auth.tryUnlink(user.sid, 'openid_flow', l4.id)).toBeFalsy();
    });
});
