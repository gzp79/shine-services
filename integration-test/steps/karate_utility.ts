import {
    binding,
    given,
    CucumberLog,
    then,
    CucumberAttachments
} from 'cucumber-tsflow';
import { KarateState, KarateCore, expect } from '$lib/karate';

@binding([CucumberLog, CucumberAttachments, KarateState])
class KarateUtility extends KarateCore {
    public constructor(
        logger: CucumberLog,
        logAttachments: CucumberAttachments,
        karate: KarateState
    ) {
        super(logger, logAttachments, karate);
    }

    @given('use karate')
    step_init() {}

    @given('wait {int}ms')
    async set_wait(ms: number) {
        await new Promise((resolve) => setTimeout(resolve, ms));
    }

    @given('def {ident} = {expr}')
    async step_storeValue(ident: string, expr: string) {
        const value = await this.karate.evalAsyncExpr(expr);
        this.log(`${ident} = ${JSON.stringify(value)}`);
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
