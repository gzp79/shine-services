import config from '../test.config';
import { MockServer } from '$lib/mock_server';
import Oauth2MockServer from '$lib/mocks/oauth2';
import OpenIdMockServer from '$lib/mocks/openid';
import { TestUser } from '$lib/user';
import { ExternalLink, getExternalLinks } from '$lib/auth_utils';
import { toBeBetween } from 'jest-extended';

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

    let mock: MockServer | undefined;

    async function stopMock() {
        if (mock) {
            await mock.stop();
            mock = undefined;
        }
    }

    async function useMockOAuth2() {
        if (mock?.name !== 'oauth2') {
            await stopMock();
        }

        if (!mock) {
            mock = await new Oauth2MockServer({ tls: config.mockTLS }).start();
        }
    }

    async function useMockOIDC() {
        if (!mock) {
            mock = await new OpenIdMockServer({
                tls: config.mockTLS,
                mockUrl: config.mockUrl,
                openidJWKS: config.openidJWKS
            }).start();
        }
    }

    afterEach(async () => {
        await mock?.stop();
        mock = undefined;
    });

    it('Sign up as quest shall not be linked', async () => {
        const user = await TestUser.createGuest();
        expect(await getExternalLinks(user.sid)).toBeEmpty();
    });

    it('Sign up with oauth2 shall not be linked', async () => {
        useMockOAuth2();
        const user = await TestUser.createLinked({ provider: 'oauth2' });
        expect(await getExternalLinks(user.sid)).toIncludeSameMembers([
            {
                ...anyLink,
                provider: 'oauth2_flow',
                userId: user.userId,
                email: user.externalUser?.email,
                name: user.externalUser?.name
            }
        ]);
    });

    it('Sign up with oidc shall not be linked', async () => {
        useMockOIDC();
        const user = await TestUser.createLinked({ provider: 'oidc' });
        expect(await getExternalLinks(user.sid)).toIncludeSameMembers([
            {
                ...anyLink,
                provider: 'openid_flow',
                userId: user.userId,
                email: user.externalUser?.email,
                name: user.externalUser?.name
            }
        ]);
    });
});
