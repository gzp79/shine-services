import { expect, KarateCore, KarateState } from '../../karate/karate';
import { Cookie } from 'tough-cookie';
import { binding, then } from 'cucumber-tsflow';
import { CucumberAttachments, CucumberLog } from 'cucumber-tsflow';

@binding([CucumberLog, CucumberAttachments, KarateState])
export class AuthCookiesSteps extends KarateCore {
    public constructor(
        logger: CucumberLog,
        logAttachments: CucumberAttachments,
        karate: KarateState
    ) {
        super(logger, logAttachments, karate);
    }

    @then("match response 'tid' cookie is valid")
    async step_validTID() {
        let cookieName = 'tid';
        let cookie = expect(
            this.karate.lastResponseCookies,
            `Missing ${cookieName} cookie`
        ).to.have.property(cookieName).subject<Cookie>;

        expect(cookie).to.have.property('secure', true);
        expect(cookie).to.have.property('httpOnly', true);
        expect(cookie).to.have.property('sameSite', 'lax');
        expect(cookie).to.have.property('path', '/identity/auth');
        expect(cookie).to.have.property('domain', 'cloud.scytta-test.com');
        expect(cookie)
            .to.have.property('expires')
            .that.is.afterTime(new Date());
    }

    @then("match response 'sid' cookie is valid")
    async step_validSID() {
        let cookieName = 'sid';
        let cookie = expect(
            this.karate.lastResponseCookies,
            `Missing ${cookieName} cookie`
        ).to.have.property(cookieName).subject<Cookie>;

        expect(cookie).to.have.property('secure', true);
        expect(cookie).to.have.property('httpOnly', true);
        expect(cookie).to.have.property('sameSite', 'lax');
        expect(cookie).to.have.property('path', '/');
        expect(cookie).to.have.property('domain', 'scytta-test.com');
        expect(cookie).to.have.property('expires', 'Infinity'); // session scoped
    }

    @then("match response 'eid' cookie is valid")
    async step_validEID() {
        let cookieName = 'eid';
        let cookie = expect(
            this.karate.lastResponseCookies,
            `Missing ${cookieName} cookie`
        ).to.have.property(cookieName).subject<Cookie>;

        expect(cookie).to.have.property('secure', true);
        expect(cookie).to.have.property('httpOnly', true);
        expect(cookie).to.have.property('sameSite', 'lax');
        expect(cookie).to.have.property('path', '/identity/auth');
        expect(cookie).to.have.property('domain', 'cloud.scytta-test.com');
        expect(cookie).to.have.property('expires', 'Infinity'); // session scoped
    }

    // Check if response contains a cookie to be remove it from the client
    @then('match response {string} cookie is removed')
    async step_Removed(cookieName: string) {
        let cookie = expect(
            this.karate.lastResponseCookies,
            `Missing ${cookieName} cookie`
        ).to.have.property(cookieName).subject<Cookie>;

        expect(cookie)
            .to.have.property('expires')
            .that.is.beforeTime(new Date());
    }

    // Check if response contains a cookie to be remove it from the client
    @then('match response {string} cookie has value {stringExpr}')
    async step_MatchValue(cookieName: string, value: string) {
        let cookie = expect(
            this.karate.lastResponseCookies,
            `Missing ${cookieName} cookie`
        ).to.have.property(cookieName).subject<Cookie>;
        expect(cookie).to.have.property('value', value);
    }
}
