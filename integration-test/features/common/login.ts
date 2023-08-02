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
import { RESTSteps } from './rest.steps';

@binding()
export class LoginSteps extends RESTSteps {
    @given('a {string} login request')
    login(provider: string) {
        const url = baseUrls["service"] + `/${provider}/login`;
        console.log(url)
        this.spec!.get(url);
    }
}