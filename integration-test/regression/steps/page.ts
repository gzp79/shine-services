import { expect, KarateState } from '$lib/karate';
import { binding, then } from 'cucumber-tsflow';

@binding([KarateState])
export class PageSteps {
    constructor(private readonly karate: KarateState) {}

    // Check if response has an exact match to the given object
    @then('match page response redirect is {stringExpr}')
    async step_redirectUrl(expected: string) {
        expect(this.karate.lastResponse).to.be.html;
        const body = this.karate.lastResponse?.text!;
        const regexp = /.*<meta http-equiv[^>]*url='([^']*)'[^>]*>.*/;
        const match = regexp.exec(body);
        expect(match![1], 'Redirect url').to.be.to.deep.equal(expected);
    }
}
