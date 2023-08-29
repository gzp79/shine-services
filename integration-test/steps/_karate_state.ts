import { Config } from './_config';
import { Response } from 'superagent';
import { Cookie } from 'tough-cookie';
import { MockServer } from './_mock_server';
import { KarateLogger } from './karate';

export class KarateState {
    private _config = new Config();
    get config() {
        return this._config;
    }

    private _properties: any = {};
    get properties(): Record<string, any> {
        return {
            ...this._config,
            response: this._lastResponse,
            responseError: this._lastResponseError,
            responseCookies: this._lastResponseCookies,
            ...this._properties
        };
    }

    setProperty(key: string, value: any) {
        this._properties[key] = value;
    }

    async evalAsyncExpr(expr: string, extraProps: object = {}): Promise<any> {
        const __karate = {
            ...this.properties,
            ...extraProps
        };
        const assignments = Object.keys(__karate).map(
            (key) => `const ${key} = __karate.${key};`
        );

        const script = `
          ${assignments.join('\n')}
            async () => { return ${expr}; }
        `;

        // import some utility to use in the eval
        const uuidModule = await import('uuid');
        const { v4: uuidV4 } = uuidModule;

        const stringUtils = await import('$lib/string_utils');
        const { createUrlQueryString, generateRandomString } = stringUtils;

        return await eval(script)();
    }

    url: string = '';
    path: string = '';
    params: Record<string, string> = {};
    cookies: Record<string, string> = {};

    private _lastResponseError: any | undefined;
    public get lastResponseError(): any | undefined {
        return this._lastResponseError;
    }

    private _lastResponse: Response | undefined;
    public get lastResponse(): Response | undefined {
        return this._lastResponse;
    }

    private _lastResponseCookies: Record<string, Cookie> | undefined;
    public get lastResponseCookies(): Record<string, Cookie> | undefined {
        return this._lastResponseCookies;
    }

    setQueryParam(key: string, value: string) {
        this.params[key] = value;
    }

    setQueryParams(kv: Record<string, string>) {
        for (const key in kv) {
            this.setQueryParam(key, kv[key]);
        }
    }

    setCookie(key: string, value: string) {
        this.cookies[key] = value;
    }

    setCookies(cookies: Record<string, string>) {
        for (const key in cookies) {
            this.setCookie(key, cookies[key]);
        }
    }

    clearRequest() {
        this.url = '';
        this.path = '';
        this.params = {};
        this.cookies = {};
    }

    clearResponse() {
        this._lastResponseError = undefined;
        this._lastResponse = undefined;
        this._lastResponseCookies = undefined;
    }

    setError(error: any) {
        this.clearRequest();
        this.clearResponse();
        this._lastResponseError = error;
    }

    setResponse(res: Response) {
        this.clearRequest();
        this.clearResponse();

        this._lastResponse = res;
        this._lastResponseCookies = (res.headers['set-cookie'] ?? [])
            .map((cookieStr: string) => Cookie.parse(cookieStr))
            .reduce((cookies: Record<string, Cookie>, cookie: Cookie) => {
                cookies[cookie.key] = cookie;
                return cookies;
            }, {});
    }

    mockServers: Record<string, MockServer> = {};

    async startMock(server: MockServer, logger?: KarateLogger) {
        if (this.mockServers[server.name]) {
            throw new Error(`Mock server '${server.name}' is already present`);
        }

        await server.start(logger);
        this.mockServers[server.name] = server;
    }

    stopMock(name: string) {
        const server = this.mockServers[name];
        if (!server) {
            throw new Error(`Mock server '${name}' is not present`);
        }

        server.stop();
        delete this.mockServers[name];
    }

    async stopAllMocks() {
        for (const name in this.mockServers) {
            const server = this.mockServers[name];
            await server.stop();
        }
        this.mockServers = {};
    }
}
