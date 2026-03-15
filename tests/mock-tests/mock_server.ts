import { expect, test } from '$fixtures/setup';
import MockSmtp from '$lib/mocks/mock_smtp';
import OAuth2MockServer from '$lib/mocks/oauth2';
import OpenIdMockServer from '$lib/mocks/openid';

test.describe('Mock server lifecycle', () => {
    test('OpenId start/stop/start shall reuse the port', async ({ api }) => {
        const mock = new OpenIdMockServer();

        await mock.start();
        expect(mock.isRunning).toBeTruthy();
        expect(await api.client.get(mock.readyUrl)).toHaveStatus(200);
        await mock.stop();
        expect(mock.isRunning).toBeFalsy();

        await mock.start();
        expect(mock.isRunning).toBeTruthy();
        expect(await api.client.get(mock.readyUrl)).toHaveStatus(200);
        await mock.stop();
        expect(mock.isRunning).toBeFalsy();
    });

    test('OAuth2 start/stop/start shall reuse the port', async ({ api }) => {
        const mock = new OAuth2MockServer();

        await mock.start();
        expect(mock.isRunning).toBeTruthy();
        expect(await api.client.get(mock.readyUrl)).toHaveStatus(200);
        await mock.stop();
        expect(mock.isRunning).toBeFalsy();

        await mock.start();
        expect(mock.isRunning).toBeTruthy();
        expect(await api.client.get(mock.readyUrl)).toHaveStatus(200);
        await mock.stop();
        expect(mock.isRunning).toBeFalsy();
    });

    test('SMTP start/stop/start shall reuse the port', async () => {
        const mock = new MockSmtp();

        await mock.start();
        expect(mock.isRunning).toBeTruthy();
        await mock.stop();
        expect(mock.isRunning).toBeFalsy();

        await mock.start();
        expect(mock.isRunning).toBeTruthy();
        await mock.stop();
        expect(mock.isRunning).toBeFalsy();
    });
});
