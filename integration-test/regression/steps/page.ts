import { expect, KarateState } from '../../karate/karate';
import { binding, given, then } from 'cucumber-tsflow';

@binding([KarateState])
export class PageSteps {
    constructor(private readonly karate: KarateState) {}

    private getPageRedirectUrl(page: string): string | undefined {
        const regexp = /.*<meta http-equiv[^>]*url='([^']*)'[^>]*>.*/;
        const match = regexp.exec(page) ?? [];
        return match[1];
    }

    private getRedirectUrl(): string | undefined {
        expect(this.karate.lastResponse).to.be.html;
        return this.getPageRedirectUrl(this.karate.lastResponse?.text ?? '');
    }

    @given('with karate plugin page')
    step_registerPage() {
        this.karate.setProperty('getPageRedirectUrl', (response: any) =>
            this.getPageRedirectUrl(response?.text ?? '')
        );
    }

    @then('match page response redirect is {stringExpr}')
    async step_redirectUrl(expected: string) {
        const url = this.getRedirectUrl();
        expect(url, 'Redirect url').to.be.to.equal(expected);
    }

    @then('match page response redirect starts with {stringExpr}')
    async step_redirectUrlStartsWith(expected: string) {
        const url = this.getRedirectUrl();
        expect(url, 'Redirect url').to.be.startWith(expected);
    }

    @then('match page response contains {stringExpr}')
    async step_pageContains(expected: string) {
        expect(this.karate.lastResponse?.text).to.contain(expected);
    }
}
