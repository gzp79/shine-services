import request from 'superagent';
import { getPageRedirectUrl } from '$lib/page_utils';
import { UserInfo, getCookies, getSessions, getUserInfo, logout } from '$lib/auth_utils';
import config from '../test.config';
import { Cookie } from 'tough-cookie';
import { ExternalUser, TestUser } from '$lib/user';
import Oauth2MockServer from '$lib/mocks/oauth2';
import { loginWithOAuth2 } from '$lib/login_utils';
import { MockServer } from '$lib/mock_server';

describe('Active session handling', () => {
    let mock!: MockServer;

    afterEach(async () => {
        await mock?.stop();
        mock = undefined!;
    });

    it('Get session without user should fail', async () => {
        // initial session for a new user
        let response = await request
            .get(config.getUrlFor('identity/api/auth/user/sessions'))
            .send()
            .catch((err) => err.response);
        expect(response.statusCode).toEqual(401);
    });

    const newSession = async (tid: string, region: string): Promise<string> => {
        const response = await request
            .get(config.getUrlFor('identity/auth/token/login'))
            .query(config.defaultRedirects)
            .set('Cookie', [`tid=${tid}`])
            .set('cf-region', region)
            .send();

        expect(response.statusCode).toEqual(200);
        expect(getPageRedirectUrl(response.text)).toEqual(config.defaultRedirects.redirectUrl);
        const cookies = getCookies(response);
        return cookies.sid.value;
    };

    it('Multiple login should create multiple sessions', async () => {
        const user = await TestUser.create([]);

        // initial session for a new user
        expect(await getSessions(user.sid!)).toIncludeSameMembers([
            { agent: '', country: null, region: null, city: null }
        ]);

        // log in from a new country (agent is not altered, to bypass fingerprint check)
        const sid2 = await newSession(user.tid!, 'r2');
        expect(await getSessions(user.sid!)).toIncludeSameMembers([
            { agent: '', country: null, region: null, city: null },
            { agent: '', country: null, region: 'r2', city: null }
        ]);

        //logout from the first session
        await logout(user.sid!, false);
        expect(await getSessions(sid2)).toIncludeSameMembers([
            { agent: '', country: null, region: 'r2', city: null }
        ]);
    });

    it('Logout from all session', async () => {
        mock = await new Oauth2MockServer({ tls: config.mockTLS }).start();

        const user = ExternalUser.newRandomUser();
        const userCookies = await loginWithOAuth2(user, true);
        const tid = userCookies.tid.value;
        const sid = userCookies.sid.value;

        // log in from a new country (agent is not altered, to bypass fingerprint check)
        await newSession(tid, 'r2');
        await newSession(tid, 'r3');
        expect(await getSessions(sid)).toIncludeSameMembers([
            { agent: '', country: null, region: null, city: null },
            { agent: '', country: null, region: 'r2', city: null },
            { agent: '', country: null, region: 'r3', city: null }
        ]);

        //logout from the all the session
        await logout(sid, true);

        //logo in again and check sessions
        const newUserCookies = await loginWithOAuth2(user, true);
        const sid2 = newUserCookies.sid.value;
        expect(await getSessions(sid2)).toIncludeSameMembers([
            { agent: '', country: null, region: null, city: null }
        ]);
    });
});
