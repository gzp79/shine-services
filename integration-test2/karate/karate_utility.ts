import {
    binding,
    given,
    CucumberLog,
    then,
    CucumberAttachments
} from 'cucumber-tsflow';
import { KarateState, KarateCore, expect } from './karate';

@binding([CucumberLog, CucumberAttachments, KarateState])
class KarateUtility extends KarateCore {
    public constructor(
        logger: CucumberLog,
        logAttachments: CucumberAttachments,
        karate: KarateState
    ) {
        super(logger, logAttachments, karate);
    }

    @given('use karate with config {string}')
    async step_init(configFile: string) {
        console.log(configFile);
        this.karate.config = new (await import(configFile)).Config();
    }

    @given('wait {int}ms')
    async set_wait(ms: number) {
        await new Promise((resolve) => setTimeout(resolve, ms));
    }

    @given('def {ident} = {expr}')
    async step_storeValue(ident: string, expr: string) {
        const value = await this.karate.evalAsyncExpr(expr);
        this.logAttach(JSON.stringify(value), 'application/json');
        this.karate.setProperty(ident, value);
    }

    @given('log {expr}')
    async step_log(expr: string) {
        const value = await this.karate.evalAsyncExpr(expr);
        this.log(value);
    }

    @then('assert {expr}')
    async step_assert(expr: string) {
        const value = await this.karate.evalAsyncExpr(expr);
        expect(value, `Expression: ${expr}`).to.be.true;
    }
}

export = KarateUtility;
