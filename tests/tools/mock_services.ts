import { getEmailLink } from '$lib/api/utils';
import MockSmtp from '$lib/mocks/mock_smtp';
import OAuth2MockServer from '$lib/mocks/oauth2';
import OpenIDMockServer from '$lib/mocks/openid';
import { StaticFileServer } from '$lib/mocks/static_file_server';
import debug from 'debug';
import path from 'path';

async function main() {
    debug.enable('test:mock:*');

    const mock_static = new StaticFileServer('game', {
        url: new URL('https://game.local-scytta.com:8092'),
        staticFilesPath: path.join(__dirname, '../..', 'dist')
    });
    await mock_static.start();

    const mock_smtp = new MockSmtp();
    await mock_smtp.start();
    mock_smtp.onMail((mail) => {
        const subject = mail.subject;
        if (subject) {
            console.log('Email subject:', subject);
        }
        const link = getEmailLink(mail);
        if (link) {
            console.log('Email link:', link);
        }
    });

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
