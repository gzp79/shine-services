import { defineParameterType } from '@cucumber/cucumber';

export enum HttpMethod {
    GET,
    PUT,
    PATCH,
    DELETE
}

defineParameterType({
    regexp: /GET|PUT|PATCH|DELETE/,
    transformer(s): HttpMethod {
        switch (s) {
            case 'GET':
                return HttpMethod.GET;
            case 'PUT':
                return HttpMethod.PUT;
            case 'PATCH':
                return HttpMethod.PATCH;
            case 'DELETE':
                return HttpMethod.DELETE;
            default:
                throw new Error(`Invalid request method: ${s}`);
        }
    },
    name: 'httpMethod'
});

export enum ServiceComponent {
    Base = 'base',
    Service = 'service',
    Api = 'api',
    Doc = 'doc'
}

defineParameterType({
    regexp: /base|service|API|doc/,
    transformer(s): ServiceComponent {
        switch (s.toLowerCase()) {
            case 'base':
                return ServiceComponent.Base;
            case 'service':
                return ServiceComponent.Service;
            case 'api':
                return ServiceComponent.Api;
            case 'doc':
                return ServiceComponent.Doc;
            default:
                throw new Error(`Invalid service component: ${s}`);
        }
    },
    name: 'serviceComponent'
});
