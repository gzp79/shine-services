import * as request from 'superagent';
import config from '../test.config';
import { createGuestUser } from '$lib/login_utils';
import { TestUser } from '$lib/user';
//import requestLogger from 'superagent-logger';

describe('User role admin', () => {
    it('General user should fail to get roles', async () => {
        const user = await TestUser.create();
        console.log(config.getUrlFor(`api/identity/identities/${user.userId}/roles`));
        const response = await request
            .get(config.getUrlFor(`api/identity/identities/${user.userId}/roles`))
            //.use(requestLogger)
            .set('Cookie', user.getCookies())
            .send();
        expect(response.statusCode).toEqual(403);
    });
});
