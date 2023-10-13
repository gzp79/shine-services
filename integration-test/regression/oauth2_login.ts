import * as request from 'superagent';
import * as os from 'os';
import { getPageRedirectUrl } from '$lib/page_utils';
import { UserInfo, getCookies, getUserInfo } from '$lib/auth_utils';
import config from '../test.config';
import { MockServer } from '$lib/mock_server';
import Oauth2MockServer from '$lib/mocks/oauth2';
import { ExternalUser } from '$lib/models/external_user';
import {
    createGuestUser,
    loginWithOAuth2,
    loginWithToken,
    startLoginWithOAuth2
} from '$lib/login_utils';
import { Cookie } from 'tough-cookie';

describe('Validate (interactive) OAuth2 auth', () => {
    let mock: MockServer | undefined;
    afterEach(async () => {
        await mock?.stop();
        mock = undefined;
    });

    it('Auth (parameters: NO, cookie: NO) should be an error', async () => {
        mock = await new Oauth2MockServer().start();

        const response = await request
            .get(config.getUrlFor('identity/auth/oauth2_flow/auth'))
            //.use(requestLogger)
            .send();

        expect(response.statusCode).toEqual(200);
        expect(getPageRedirectUrl(response.text)).toEqual(
            'http://web.scytta-test.com:8080/error?type=authError&status=400'
        );
        expect(response.text).toContain('&quot;MissingExternalLogin&quot;');

        const cookies = getCookies(response);
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeClearCookie();
        expect(cookies.eid).toBeClearCookie();
    });

    it('Auth (parameters: VALID, cookie: NO) should be an error', async () => {
        mock = await new Oauth2MockServer().start();
        const { authParams } = await startLoginWithOAuth2();
        const response = await request
            .get(config.getUrlFor('identity/auth/oauth2_flow/auth'))
            .query({
                code: ExternalUser.newRandomUser().toCode(),
                state: authParams.state
            })
            //.use(requestLogger)
            .send();

        expect(response.statusCode).toEqual(200);
        expect(getPageRedirectUrl(response.text)).toEqual(
            'http://web.scytta-test.com:8080/error?type=authError&status=400'
        );
        expect(response.text).toContain('&quot;MissingExternalLogin&quot;');

        const cookies = getCookies(response);
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeClearCookie();
        expect(cookies.eid).toBeClearCookie();
    });

    it('Auth (parameters: NO, cookie: VALID) should be an error', async () => {
        mock = await new Oauth2MockServer().start();
        const { authParams, eid } = await startLoginWithOAuth2();
        const response = await request
            .get(config.getUrlFor('identity/auth/oauth2_flow/auth'))
            .set('Cookie', [`eid=${eid.value}`])
            //.use(requestLogger)
            .send();

        expect(response.statusCode).toEqual(200);
        expect(getPageRedirectUrl(response.text)).toEqual(
            config.defaultRedirects.errorUrl + '?type=invalidInput&status=400'
        );
        expect(response.text).toContain('Failed to deserialize query string');

        const cookies = getCookies(response);
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeClearCookie();
        expect(cookies.eid).toBeClearCookie();
    });

    it('Auth (parameters: INVALID state, cookie: VALID) should be an error', async () => {
        mock = await new Oauth2MockServer().start();
        const { authParams, eid } = await startLoginWithOAuth2();
        const response = await request
            .get(config.getUrlFor('identity/auth/oauth2_flow/auth'))
            .query({
                code: ExternalUser.newRandomUser().toCode(),
                state: 'invalid'
            })
            .set('Cookie', [`eid=${eid.value}`])
            //.use(requestLogger)
            .send();

        expect(response.statusCode).toEqual(200);
        expect(getPageRedirectUrl(response.text)).toEqual(
            config.defaultRedirects.errorUrl + '?type=authError&status=400'
        );
        expect(response.text).toContain('&quot;InvalidCSRF&quot;');

        const cookies = getCookies(response);
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeClearCookie();
        expect(cookies.eid).toBeClearCookie();
    });

    it('Auth (parameters: INVALID code, cookie: VALID) should be an error', async () => {
        mock = await new Oauth2MockServer().start();
        const { authParams, eid } = await startLoginWithOAuth2();
        const response = await request
            .get(config.getUrlFor('identity/auth/oauth2_flow/auth'))
            .query({
                code: 'invalid',
                state: authParams.state
            })
            .set('Cookie', [`eid=${eid.value}`])
            //.use(requestLogger)
            .send();

        expect(response.statusCode).toEqual(200);
        expect(getPageRedirectUrl(response.text)).toEqual(
            config.defaultRedirects.errorUrl + '?type=authError&status=500'
        );
        expect(response.text).toContain('Server returned empty error response');

        const cookies = getCookies(response);
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeClearCookie();
        expect(cookies.eid).toBeClearCookie();
    });

    it('Auth with failing 3rd party (token service) should be an error', async () => {
        // intentionally not started: mock = await new Oauth2MockServer().start();
        const { authParams, eid } = await startLoginWithOAuth2();
        const response = await request
            .get(config.getUrlFor('identity/auth/oauth2_flow/auth'))
            .query({
                code: ExternalUser.newRandomUser().toCode(),
                state: authParams.state
            })
            .set('Cookie', [`eid=${eid.value}`])
            //.use(requestLogger)
            .send();

        expect(response.statusCode).toEqual(200);
        expect(getPageRedirectUrl(response.text)).toEqual(
            config.defaultRedirects.errorUrl + '?type=authError&status=500'
        );
        console.log('current os.platform:', os.platform());
        if (os.platform() === 'win32') {
            expect(response.text).toContain(
                'No connection could be made because the target machine actively refused it.'
            );
        } else {
            expect(response.text).toContain('Connection refused');
        }

        const cookies = getCookies(response);
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeClearCookie();
        expect(cookies.eid).toBeClearCookie();
    });
});

describe('Validate (interactive) OAuth2 login', () => {
    let mock!: MockServer;

    beforeEach(async () => {
        mock = await new Oauth2MockServer().start();
    });

    afterEach(async () => {
        await mock?.stop();
        mock = undefined!;
    });

    it('Login with a user and with a session should be an error', async () => {
        const { sid } = await createGuestUser();

        const response = await request
            .get(config.getUrlFor('identity/auth/oauth2_flow/login'))
            .query({ ...config.defaultRedirects })
            .set('Cookie', [`sid=${sid.value}`])
            //.use(requestLogger)
            .send();

        expect(response.statusCode).toEqual(200);
        expect(getPageRedirectUrl(response.text)).toEqual(
            config.defaultRedirects.errorUrl + '?type=logoutRequired&status=400'
        );
        expect(response.text).toContain('&quot;LogoutRequired&quot;');

        const authCookies = getCookies(response);
        expect(authCookies.tid).toBeClearCookie();
        expect(authCookies.sid).toBeValidSID();
        expect(authCookies.sid.value).toEqual(sid.value);
        expect(authCookies.eid).toBeClearCookie();
    });

    it('Login with a user and with a token should be a success', async () => {
        const { tid } = await createGuestUser();

        const response = await request
            .get(config.getUrlFor('identity/auth/oauth2_flow/login'))
            .query({ ...config.defaultRedirects })
            .set('Cookie', [`tid=${tid.value}`])
            //.use(requestLogger)
            .send();

        expect(response.statusCode).toEqual(200);
        const redirectUrl = getPageRedirectUrl(response.text);
        expect(redirectUrl).toStartWith('http://mock.localhost.com:8090/oauth2/authorize');

        const authCookies = getCookies(response);
        expect(authCookies.tid).toBeClearCookie();
        expect(authCookies.sid).toBeClearCookie();
        expect(authCookies.eid).toBeValidEID();
    });

    it('Login with a new user should register the user', async () => {
        const user = ExternalUser.newRandomUser();
        const cookies = await loginWithOAuth2(user);
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeValidSID();
        expect(cookies.eid).toBeClearCookie();

        const userInfo = await getUserInfo(cookies.sid);
        expect(userInfo.name).toEqual(user.name);
    });

    it('Login with a new user and with false rememberMe should register the user', async () => {
        const user = ExternalUser.newRandomUser();
        const cookies = await loginWithOAuth2(user, false);
        expect(cookies.tid).toBeClearCookie();
        expect(cookies.sid).toBeValidSID();
        expect(cookies.eid).toBeClearCookie();

        const userInfo = await getUserInfo(cookies.sid);
        expect(userInfo.name).toEqual(user.name);
    });

    it('Login with a new user and with true rememberMe should register the user', async () => {
        const user = ExternalUser.newRandomUser();
        const cookies = await loginWithOAuth2(user, true);
        expect(cookies.tid).toBeValidTID();
        expect(cookies.sid).toBeValidSID();
        expect(cookies.eid).toBeClearCookie();

        const userInfo = await getUserInfo(cookies.sid);
        expect(userInfo.name).toEqual(user.name);
    });
});

describe('(Interactive) OAuth2 flow', () => {
    let mock!: MockServer;
    let user!: ExternalUser;
    let userCookies!: Record<string, Cookie>;
    let userInfo!: Omit<UserInfo, 'sessionLength'>;

    beforeEach(async () => {
        mock = await new Oauth2MockServer().start();
        user = ExternalUser.newRandomUser();
        userCookies = await loginWithOAuth2(user, true);
        const { sessionLength, ...info } = await getUserInfo(userCookies.sid);
        userInfo = info;
        expect(userInfo.name).toEqual(user.name);
    });

    afterEach(async () => {
        await mock.stop();
        mock = undefined!;
        user = undefined!;
        userCookies = undefined!;
        userInfo = undefined!;
    });

    it('Login with the same user should be a success', async () => {
        const newUserCookies = await loginWithOAuth2(user);
        expect(newUserCookies.sid.value, 'It shall be a new session').not.toEqual(
            userCookies.sid.value
        );

        const newUserInfo = await getUserInfo(newUserCookies.sid);
        expect(newUserInfo).toEqual(expect.objectContaining(userInfo));
    });

    it('Login with the token should be a success', async () => {
        const newUserCookies = await loginWithToken(userCookies.tid);
        expect(newUserCookies.sid.value, 'It shall be a new session').not.toEqual(
            userCookies.sid.value
        );

        const newUserInfo = await getUserInfo(newUserCookies.sid);
        expect(newUserInfo).toEqual(expect.objectContaining(userInfo));
    });
});
