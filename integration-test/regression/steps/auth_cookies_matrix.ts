import { Config, KarateCore, KarateLogger, KarateState } from '$lib/karate';
import { request, expect } from '$lib/karate';
import { binding, given, then } from 'cucumber-tsflow';
import { CucumberAttachments, CucumberLog } from 'cucumber-tsflow';
import { Cookie } from 'tough-cookie';

let data: Record<string, string> = {};

async function createData(
    config: Config,
    logger: KarateLogger
): Promise<Record<string, string>> {
    // some ugly hack to create cookies only once and used by the scenario outline
    if (!!data['ok']) {
        logger.log('Using cached data');
        logger.logAttach(JSON.stringify(data), 'application/json');
        return data;
    }

    data['ok'] = 'Generating...';
    logger.log('Generating data');
    const create_cookies = async (): Promise<Record<string, string>> => {
        let cookies: Record<string, string> = {};

        const resToken = await request(config.serviceUrl)
            .get('/identity/auth/token/login')
            .query({ rememberMe: true })
            .send();
        expect(resToken).to.be.status(200);
        const cookieToken = (resToken.headers['set-cookie'] ?? [])
            .map((cookieStr: string) => Cookie.parse(cookieStr))
            .reduce((cookies: Record<string, Cookie>, cookie: Cookie) => {
                cookies[cookie.key] = cookie;
                return cookies;
            }, {});
        cookies.tid = cookieToken.tid.value;
        cookies.sid = cookieToken.tid.value;

        const resOAuth = await request(config.serviceUrl)
            .get('/identity/auth/oauth2_flow/link')
            .set('Cookie', [`sid=${cookies.sid}`])
            .send();
        expect(resOAuth).to.be.status(200);
        const cookieOAuth = (resToken.headers['set-cookie'] ?? [])
            .map((cookieStr: string) => Cookie.parse(cookieStr))
            .reduce((cookies: Record<string, Cookie>, cookie: Cookie) => {
                cookies[cookie.key] = cookie;
                return cookies;
            }, {});
        cookies.eid = cookieOAuth.eid.value;

        expect(cookies.tid).to.be.a('string');
        expect(cookies.sid).to.be.a('string');
        expect(cookies.eid).to.be.a('string');

        return cookies;
    };

    const c1 = await create_cookies();
    data['t'] = c1.tid;
    data['s'] = c1.sid;
    data['e'] = c1.eid;
    data['ts'] = 'invalid'.concat(c1.tid.slice(7));
    data['ss'] = 'invalid'.concat(c1.sid.slice(7));
    data['es'] = 'invalid'.concat(c1.eid.slice(7));

    const c2 = await create_cookies();
    data['t2'] = c2.tid;

    const c3 = await create_cookies();
    data['s2'] = c3.sid;

    const c4 = await create_cookies();
    data['e2'] = c4.eid;

    data['ok'] = 'Done.';
    logger.logAttach(JSON.stringify(data), 'application/json');
    return data;
}

@binding([CucumberLog, CucumberAttachments, KarateState])
export class AuthCookieMatrixSteps extends KarateCore {
    public constructor(
        logger: CucumberLog,
        logAttachments: CucumberAttachments,
        karate: KarateState
    ) {
        super(logger, logAttachments, karate);
    }

    @given('auth cookie matrix {string} {string} {string}')
    async step_setupCookies(tid: string, sid: string, eid: string) {
        const data = await createData(this.karate.config, this);

        for (const x of [
            [tid, 'tid', 't'],
            [sid, 'sid', 's'],
            [eid, 'eid', 'e']
        ]) {
            switch (x[0]) {
                case '+':
                    this.karate.setCookie(x[1], data[x[2]]);
                    break;
                case '-':
                    /* noop */ break;
                case '!':
                    this.karate.setCookie(x[1], data[x[2] + '2']);
                    break;
                case 's':
                    this.karate.setCookie(x[1], data[x[2] + 's']);
                    break;
                default:
                    throw new Error(
                        `Unhandled cookie mod for ${x[1]}: ${x[0]}`
                    );
            }
        }
    }

    @then('match auth cookie matrix {string}')
    step_checkCookies(expected: string) {}
}
