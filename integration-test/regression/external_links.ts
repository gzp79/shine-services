import config from '../test.config';
import OAuth2MockServer from '$lib/mocks/oauth2';
import OpenIdMockServer from '$lib/mocks/openid';
import { ExternalUser, TestUser } from '$lib/user';
import { ExternalLink, getExternalLinks } from '$lib/auth_utils';
import { linkWithOAuth2, linkWithOpenId } from '$lib/login_utils';

describe('External links', () => {
    const now = new Date().getTime();
    const linkRange = [new Date(now - 60 * 1000), new Date(now + 60 * 1000)];
    const anyLink: ExternalLink = {
        userId: expect.toBeString(),
        provider: expect.toBeString(),
        providerUserId: expect.toBeString(),
        linkedAt: expect.toBeBetween(linkRange[0], linkRange[1]),
        name: expect.anything(),
        email: expect.anything()
    };

    let mockOAuth2: OAuth2MockServer;
    let mockOpenId: OpenIdMockServer;

    beforeEach(async () => {
        mockOAuth2 = new OAuth2MockServer({
            tls: config.mockTLS,
            url: config.mockUrl
        });
        await mockOAuth2.start();

        mockOpenId = new OpenIdMockServer({
            tls: config.mockTLS,
            url: config.mockUrl,
            jwks: config.openidJWKS
        });
        await mockOpenId.start();
    });

    afterEach(async () => {
        await mockOAuth2.stop();
        mockOAuth2 = undefined!;
        await mockOpenId.stop();
        mockOpenId = undefined!;
    });

    it('Sign up as guest shall not be linked', async () => {
        const user = await TestUser.createGuest();
        expect(await getExternalLinks(user.sid)).toBeEmpty();
    });

    it('Sign up with OAuth2 shall create a link and delete link shall work', async () => {
        const user = await TestUser.createLinked(mockOAuth2);
        expect(await getExternalLinks(user.sid)).toIncludeSameMembers([
            {
                ...anyLink,
                provider: 'oauth2_flow',
                userId: user.userId,
                email: user.externalUser?.email,
                name: user.externalUser?.name
            }
        ]);

        //todo: unlink
        //expect(await getExternalLinks(user.sid)).toBeEmpty();
    });

    it('Sign up with OpenId shall create a link and delete link shall work', async () => {
        const user = await TestUser.createLinked(mockOpenId);
        expect(await getExternalLinks(user.sid)).toIncludeSameMembers([
            {
                ...anyLink,
                provider: 'openid_flow',
                userId: user.userId,
                email: user.externalUser?.email,
                name: user.externalUser?.name
            }
        ]);

        //todo: unlink
        //expect(await getExternalLinks(user.sid)).toBeEmpty();
    });

    it('Link to multiple provider and delete links shall work', async () => {
        const user = await TestUser.createGuest();
        expect(await getExternalLinks(user.sid)).toBeEmpty();

        const l1 = ExternalUser.newRandomUser();
        await linkWithOAuth2(mockOAuth2, user.sid, l1);
        const links1 = await getExternalLinks(user.sid);
        const t1 = links1.find((l) => l.userId === l1.id)!;
        expect(links1).toIncludeSameMembers([
            { ...anyLink, provider: 'oauth2_flow', email: l1.email, name: l1.name }
        ]);

        const l2 = ExternalUser.newRandomUser();
        await linkWithOpenId(mockOpenId, user.sid, l2);
        const links2 = await getExternalLinks(user.sid);
        const t2 = links2.find((l) => l.userId === l2.id)!;
        expect(links2).toIncludeSameMembers([
            { ...anyLink, provider: 'oauth2_flow', email: l1.email, name: l1.name },
            { ...anyLink, provider: 'openid_flow', email: l2.email, name: l2.name }
        ]);

        const l3 = ExternalUser.newRandomUser();
        await linkWithOAuth2(mockOAuth2, user.sid, l3);
        expect(await getExternalLinks(user.sid)).toIncludeSameMembers([
            { ...anyLink, provider: 'oauth2_flow', email: l1.email, name: l1.name },
            { ...anyLink, provider: 'openid_flow', email: l2.email, name: l2.name },
            { ...anyLink, provider: 'oauth2_flow', email: l3.email, name: l3.name }
        ]);

        const l4 = ExternalUser.newRandomUser();
        await linkWithOpenId(mockOpenId, user.sid, l4);
        expect(await getExternalLinks(user.sid)).toIncludeSameMembers([
            { ...anyLink, provider: 'oauth2_flow', email: l1.email, name: l1.name },
            { ...anyLink, provider: 'openid_flow', email: l2.email, name: l2.name },
            { ...anyLink, provider: 'oauth2_flow', email: l3.email, name: l3.name },
            { ...anyLink, provider: 'openid_flow', email: l4.email, name: l4.name }
        ]);

        //delete t1
        //delete t2
    });
});
