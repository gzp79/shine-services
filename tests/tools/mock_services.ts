import { getEmailLink } from '$lib/api/utils';
import { DnsServer } from '$lib/mocks/dns_server';
import MockSmtp from '$lib/mocks/mock_smtp';
import OAuth2MockServer from '$lib/mocks/oauth2';
import OpenIDMockServer from '$lib/mocks/openid';
import { StaticFileServer } from '$lib/mocks/static_file_server';
import debug from 'debug';
import path from 'path';

async function main() {
    debug.enable('test:mock:*');

    const mock_game = new StaticFileServer('game', {
        url: new URL('https://game.local.scytta.com:8092'),
        // Serve the web client build directly; the manifest and version prefix are
        // synthesized in memory to emulate the deployed bucket.
        staticFilesPath: path.join(__dirname, '../..', 'client/web/dist'),
        latestVersion: 'custom'
    });
    await mock_game.start();

    const mock_assets = new StaticFileServer('assets', {
        url: new URL('https://assets.local.scytta.com:8093'),
        staticFilesPath: path.join(__dirname, '../../..', 'shine-assets/generated/assets')
    });
    await mock_assets.start();

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

    const mock_dns = new DnsServer({
        zone: 'local.scytta.com',
        upstream: '8.8.8.8',
        port: 53
    });
    await mock_dns.start();
}

main()
    .then(() => {
        console.log('Done.');
    })
    .catch((e) => {
        console.error(e);
        process.exit(1);
    });
