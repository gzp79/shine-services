import { Config } from './_config';
import { Response } from 'superagent';
import { Cookie } from 'cookiejar';

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

    setProperty(key: string, value: any) {
        this._properties[key] = value;
    }

    evalExpr(expr: string, extraProps: object = {}): any {
        const __karate = {
            ...this.properties,
            ...extraProps
        };
        const assignments = Object.keys(__karate).map((key) => {
            return `const ${key} = __karate.${key};`;
        });
        const script = `
            ${assignments.join('\n')}
            () => { return ${expr}; }
        `;
        return eval(script)();
    }

    async evalAsyncExpr(expr: string, extraProps: object = {}): Promise<any> {
        const __karate = {
            ...this.properties,
            ...extraProps
        };
        const assignments = Object.keys(__karate).map((key) => {
            return `const ${key} = __karate.${key};`;
        });
        const script = `
            ${assignments.join('\n')}
            async () => { return ${expr}; }
        `;
        return await eval(script)();
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
            .map((cookieStr: string) => new Cookie(cookieStr))
            .reduce((cookies: Record<string, Cookie>, cookie: Cookie) => {
                cookies[cookie.name] = cookie;
                return cookies;
            }, {});
    }
}
