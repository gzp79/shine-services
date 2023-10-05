import {
    given,
    when,
    then,
    CucumberLog,
    binding,
    CucumberAttachments
} from 'cucumber-tsflow';
import { SuperAgentRequest } from 'superagent';
import { expect, request, HttpMethod, KarateState, KarateCore } from './karate';

@binding([CucumberLog, CucumberAttachments, KarateState])
class KarateRequests extends KarateCore {
    public constructor(
        logger: CucumberLog,
        logAttachments: CucumberAttachments,
        karate: KarateState
    ) {
        super(logger, logAttachments, karate);
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

    @given('param {ident} = {stringExpr}')
    step_param(ident: string, value: string) {
        this.karate.setQueryParam(ident, value);
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
        this.log(`Request url: ${req.url}`);
        this.logAttach(JSON.stringify(this.karate.params), 'application/json');
        this.logAttach(JSON.stringify(cookies), 'application/json');

        let resp;
        try {
            resp = await req.send();
        } catch (error: any) {
            this.log(
                `Exception occurred during ${method} request: ${JSON.stringify(
                    error
                )}`
            );
            this.karate.setError(error);
        }

        if (resp) {
            this.log(`Response status: ${resp.status}`);
            this.logAttach(JSON.stringify(resp.headers), 'application/json');
            if (!resp.noContent) {
                this.logAttach(
                    resp.text,
                    resp.get('content-type') ?? 'text/plain'
                );
            }
            this.karate.setResponse(resp);
        }
    }

    @then('status {int}')
    async step_status(expectedStatusCode: number) {
        expect(this.karate.lastResponseError).to.be.undefined;
        expect(this.karate.lastResponse).to.have.status(expectedStatusCode);
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

export = KarateRequests;
