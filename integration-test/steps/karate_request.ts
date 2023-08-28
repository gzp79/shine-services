import { given, when, then, CucumberLog, binding } from 'cucumber-tsflow';
import { SuperAgentRequest } from 'superagent';
import { expect, request, HttpMethod, KarateState, KarateCore } from '$lib/karate';

@binding([CucumberLog, KarateState])
export class KarateRequests extends KarateCore {
    public constructor(logger: CucumberLog, karate: KarateState) {
        super(logger, karate);
    }

    @given('url {stringExpr}')
    step_url(url: string) {
        this.karate.url = url;
    }

    @given('path {stringExpr}')
    step_path(path: string) {
        this.karate.path = path;
    }

    @given('params {paramExpr}')
    step_params(expr: Record<string, string>) {
        this.karate.setQueryParams(expr);
    }

    @given('cookies {paramExpr}')
    step_cookies(expr: Record<string, string>) {
        this.karate.setCookies(expr);
    }

    @when('method {HttpMethod}')
    async step_method(method: HttpMethod) {
        let cookies = [];
        for (const k in this.karate.cookies) {
            cookies.push(`${k}=${this.karate.cookies[k]}`);
        }
        let req: SuperAgentRequest = (request(this.karate.url) as any)
            [
                // the name of the function to call is the same as the method in lower case
                method.toLowerCase()
            ](this.karate.path)
            .query(this.karate.params)
            .set('Cookie', cookies);
        this.log(`url: ${req.url}`);
        this.log(`query: ${JSON.stringify(this.karate.params)}`);
        this.log(`cookies: ${cookies}`);

        try {
            this.karate.setResponse(await req.send());
            this.log(`Status: ${this.karate.lastResponse?.status}`);
            this.log(`Headers: ${this.karate.lastResponse?.headers}`);
            this.log(`Response: ${this.karate.lastResponse?.text}`);
        } catch (error: any) {
            this.karate.setError(error /*, error.response*/);
            this.log(`Exception occurred during ${method} request`, error);
        }
    }

    @then('status {int}')
    async step_status(expectedStatusCode: number) {
        expect(this.karate.lastResponseError).to.be.undefined;
        expect(this.karate.lastResponse).to.have.status(
            expectedStatusCode
        );
    }

    // Check if response has an exact match to the given object
    @then('match json response == {jsonExpr}')
    async step_jsonExact(expected: object) {
        expect(this.karate.lastResponse).to.be.json;
        const body = this.karate.lastResponse?.body;
        expect(body).to.be.to.deep.equal(expected);
    }

    // Check if response contains only the given properties in any order
    @then('match json response contains only {jsonExpr}')
    async step_jsonUnorderedExact(expected: object) {
        expect(this.karate.lastResponse).to.be.json;
        const body = this.karate.lastResponse?.body;
        expect(body).to.be.to.deep.equalInAnyOrder(expected);
    }
}
