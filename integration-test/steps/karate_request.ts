import { Given, When, Then, defineParameterType } from '@cucumber/cucumber';
import chai, { expect, request } from 'chai';
import * as config from '$lib/config';
import * as path from 'path';
import chaiMatchPattern from 'chai-match-pattern';
import deepEqualInAnyOrder from 'deep-equal-in-any-order';
import chaiHttp from 'chai-http';
import { SuperAgentRequest, Response } from 'superagent';

chai.use(chaiMatchPattern);
chai.use(deepEqualInAnyOrder);
chai.use(chaiHttp);
const _ = chaiMatchPattern.getLodashModule();

const HttpMethodList = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE'] as const;
type HttpMethodTuple = typeof HttpMethodList;
type HttpMethod = HttpMethodTuple[number];
defineParameterType({
    name: 'HttpMethod',
    regexp: new RegExp(`${HttpMethodList.join('|')}`),
    transformer: (method) => method
});

defineParameterType({
    name: 'json',
    regexp: new RegExp('{.*}$'),
    transformer: (json) => JSON.parse(json)
});

class KarateRequest {
    urlProperties: Record<string, string> = {};
    url: string = '';
    path: string = '';
    lastError: any | undefined;
    lastResponse: Response | undefined;
}
export const karate = new KarateRequest();

Given('configure url {string}={string}', function (key: string, value: string) {
    karate.urlProperties[key] = value;
});

Given('url {string}', function (url: string) {
    karate.url = url;
});

Given('path {string}', function (path: string) {
    karate.path = path;
});

When('method {HttpMethod}', async function (method: HttpMethod) {
    karate.lastError = undefined;
    karate.lastResponse = undefined;

    const urlParams = {
        appDomain: config.appDomain,
        serviceDomain: config.serviceDomain,
        serviceUrl: config.serviceUrl,
        identityUrl: config.identityUrl,
        ...karate.urlProperties
    };
    const url = karate.url.format(urlParams);
    const urlPath = karate.path.format(urlParams);

    let request: SuperAgentRequest = (chai.request(url) as any)[
        // the name of the function to call is the same as the method in lower case
        method.toLowerCase()
    ](urlPath);
    this.log(`url: ${request.url}`);

    try {
        karate.lastResponse = await request.send();
    } catch (error) {
        karate.lastError = error;
        this.log(`Exception occurred while ${method}: ${error}`);
    }
});

Then('status {int}', async function (expectedStatusCode: number) {
    expect(karate.lastResponse).to.have.status(expectedStatusCode);
});

// Check if response has an exact match to the given object
Then(
    'match json response == {json}',
    async function (expected: any) {
        expect(karate.lastResponse).to.be.json;
        expect(karate.lastResponse?.body).to.be.to.deep.equal(expected);
    }
);

// Check if response contains only the given properties in any order
Then(
    'match json response contains only {json}',
    async function (expected: any) {
        expect(karate.lastResponse).to.be.json;
        expect(karate.lastResponse?.body).to.be.to.deep.equalInAnyOrder(
            expected
        );
    }
);
