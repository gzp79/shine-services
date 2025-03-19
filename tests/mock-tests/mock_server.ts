import { expect, test } from '$fixtures/setup';
import { ApiRequest } from '$lib/api/api';
import MockSmtp from '$lib/mocks/mock_smtp';
import OpenIdMockServer from '$lib/mocks/openid';

test.describe('OpenId mock server', () => {
    test('Test mock server', async () => {
        const mock = new OpenIdMockServer();
        await mock.start();
        const response = await ApiRequest.get(mock!.getUrlFor('/.well-known/openid-configuration'));
        expect(response).toHaveStatus(200);
        await mock?.stop();
        expect(mock.isRunning).toBeFalsy();
    });

    test('Test SMTP server', async () => {
        const mock = new MockSmtp();
        await mock.start();
        const mWaiter = mock.waitMail();
        await mock.stop();
        expect(mock.isRunning).toBeFalsy();
        try {
            await mWaiter;
            // should have been rejected
            expect(true).toBeFalsy();
        } catch (e) {
            expect(e).toBeInstanceOf(Error);
            expect((e as Error).message).toEqual('Mail server stopped.');
        }
    });
});
