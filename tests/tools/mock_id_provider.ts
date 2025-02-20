import MockSmtp from '$lib/mocks/mock_smtp';
import OAuth2MockServer from '$lib/mocks/oauth2';
import OpenIDMockServer from '$lib/mocks/openid';
import debug from 'debug';

async function main() {
    debug.enable('test:mock:*');

    const mock_smtp = new MockSmtp();
    await mock_smtp.start();

    const mock_oath = new OAuth2MockServer();
    await mock_oath.start();

    const mock_oidc = new OpenIDMockServer();
    await mock_oidc.start();
}

main()
    .then(() => {
        console.log('Done.');
    })
    .catch((e) => {
        console.error(e);
    });
