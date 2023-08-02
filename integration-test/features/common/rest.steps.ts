import {
    binding,
    given,
    then,
    when,
    after,
    before,
    beforeAll
} from 'cucumber-tsflow';
import pactum from 'pactum';
import Spec from 'pactum/src/models/Spec';
import { baseUrls } from './config';
import { HttpMethod, ServiceComponent } from './parameter_types';

@binding()
export class RESTSteps {
    spec: Spec | null = null;

    @beforeAll()
    beforeAll() {
        console.log('before all');
        pactum.request.setDefaultTimeout(10000);
    }

    @before()
    before() {
        this.spec = pactum.spec();
    }

    @after()
    after() {
        this.spec!.end();
        this.spec = null;
    }

    @given('a {httpMethod} request to the {serviceComponent} at {string}')
    request(method: HttpMethod, component: ServiceComponent, endpoint: string) {
        if (!endpoint.startsWith('/')) endpoint = '/' + endpoint;
        const url = baseUrls[component] + endpoint;
        switch (method) {
            case HttpMethod.GET:
                this.spec!.get(url);
                break;
            case HttpMethod.PUT:
                this.spec!.put(url);
                break;
            case HttpMethod.PATCH:
                this.spec!.patch(url);
                break;
            case HttpMethod.DELETE:
                this.spec!.delete(url);
                break;
        }
    }

    @given('with user session {string}')
    withUserSession(cookie: string) {
        this.spec!.withCookies('sid', cookie);
    }

    /*Given(/the body is/, function (body) {
	  try {
		  this.spec.withJson(JSON.parse(body));
	  } catch(error) {
		  spec.withBody(body);
	  }
});*/

    @when('the response is received')
    async toss() {
        await this.spec!.toss();
    }

    @then('the response should have a status {int}')
    assertStatus(code: number) {
        this.spec!.response().should.have.status(code);
    }

    @then('the response should have a body')
    body(body: string) {
        this.spec!.response().should.have.body(body);
    }

    @then('the response should have an body with {string}')
    bodyContain(value: string) {
        this.spec!.response().should.have.bodyContains(value);
    }

    @then('the response should have a json')
    jsonBody(json: string) {
        this.spec!.response().should.have.json(JSON.parse(json));
    }

    @then('the response should have a json at {string}')
    jsonBodyAt(path: string, json: string) {
        this.spec!.response().should.have.json(path, JSON.parse(json));
    }

    @then('the response should have a json like')
    jsonBodyLike(json: string) {
        this.spec!.response().should.have.jsonLike(JSON.parse(json));
    }

    @then('the response should have a json at {string} like ')
    jsonBodyLikeAt(path: string, json: string) {
        this.spec!.response().should.have.jsonLike(path, JSON.parse(json));
    }

    @then('the response should have a json schema')
    jsonBodySchema(json: string) {
        this.spec!.response().should.have.jsonSchema(JSON.parse(json));
    }

    @then('the response should have a json schema at {string}')
    jsonBodySchemaAt(path: string, json: string) {
        this.spec!.response().should.have.jsonSchema(path, JSON.parse(json));
    }
}
