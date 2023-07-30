import { binding, given, then, when, after } from 'cucumber-tsflow';
import pactum from 'pactum';
import { baseApiUrl } from './config';


@binding()
export class RESTSteps {
    spec = pactum.spec();

    @given('a {string} request to (.*)')
    request(method: string, endpoint: string) {
      let url = baseApiUrl + endpoint
      console.log("url", url)
      switch (method.toLowerCase()) {
        case "get": this.spec.get(url); break;
        default: throw new Error(`Invalid method: ${method}`);
      }
    }

    @given('with user session {string}')
    withUserSession(cookie: string) {
        this.spec.withCookies('sid', cookie);
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
        await this.spec.toss();
    }

    @then('the response should have a status {int}')
    assertStatus(code: number) {
        this.spec.response().should.have.status(code);
    }

    @after()
    after() {
        this.spec.end();
    }
}
