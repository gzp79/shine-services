import { OptionalSchema } from '$lib/schema_utils';
import { convertKeysToLowerCase, removeUndefinedValues } from '$lib/utils';
import { randomUUID } from 'crypto';
import debug from 'debug';
import { APIRequestContext, APIResponse, request } from 'playwright';
import { z } from 'zod';

const log = debug('test:request');

/// Any payload used to test edge cases, model validation, etc.
export type Unstructured = {
    type: 'unstructured';
    rawData: unknown;
};

/// Construct an unstructured payload
export function unstructured(rawData: unknown): Unstructured {
    return { type: 'unstructured', rawData };
}

/// Payload that can be a structured input or an Unstructured type
export type Payload<Structured> = Unstructured | Structured;

export type Cookie = {
    name: string;
    value: string;
    domain: string;
    path: string;
    expires?: Date;
    httpOnly: boolean;
    secure: boolean;
    sameSite: 'Strict' | 'Lax' | 'None' | string;
};

export const ProblemSchema = z.object({
    status: z.number(),
    type: z.string(),
    instance: OptionalSchema(z.string()),
    detail: z.string(),
    extension: z.any(),
    sensitive: z.any()
});
export type Problem = z.infer<typeof ProblemSchema>;

export type ApiMethod = 'get' | 'post' | 'put' | 'patch' | 'delete';
export type ApiParams = Record<string, string | number | boolean>;

function parseSameSite(value: string): string {
    switch (value.toLowerCase()) {
        case 'strict':
            return 'Strict';
        case 'lax':
            return 'Lax';
        case 'none':
            return 'None';
    }
    return value;
}

function parseCookie(cookieString: string): Cookie {
    const parts = cookieString.split(';').map((part) => part.trim());
    const [name, value] = parts[0].split('=');

    const cookie = { name, value } as Cookie;

    for (let i = 1; i < parts.length; i++) {
        const [key, val] = parts[i].split('=');
        switch (key.toLowerCase()) {
            case 'path':
                cookie.path = val;
                break;
            case 'domain':
                cookie.domain = val;
                break;
            case 'expires':
                cookie.expires = new Date(val);
                break;
            case 'secure':
                cookie.secure = true;
                break;
            case 'httponly':
                cookie.httpOnly = true;
                break;
            case 'samesite':
                cookie.sameSite = parseSameSite(val) as 'Strict' | 'Lax' | 'None';
                break;
        }
    }

    return cookie;
}

export class ApiResponse {
    private _context: APIRequestContext;
    private _response: APIResponse;

    constructor(context: APIRequestContext, response: APIResponse) {
        this._response = response;
        this._context = context;
    }

    public status(): number {
        return this._response.status();
    }

    public headers(): Record<string, string | string[]> {
        return this._response.headersArray().reduce(
            (acc, header) => {
                const name = header.name.toLowerCase();
                if (header.name in acc) {
                    if (typeof acc[name] === 'string') {
                        acc[name] = [acc[name], header.value];
                    } else {
                        acc[name].push(header.name);
                    }
                } else {
                    acc[name] = header.value;
                }
                return acc;
            },
            {} as Record<string, string | string[]>
        );
    }

    public cookies(): Record<string, Cookie> {
        // await this._context.storageState()).cookies is not appropriate for me as it contains only the live (non-expired) cookies, the browser would
        // store. In the test we need the expiration time and similar info from the raw set-cookie response.

        return this._response
            .headersArray()
            .filter((x) => x.name.toLowerCase() === 'set-cookie')
            .map((x) => parseCookie(x.value))
            .reduce(
                (acc, cookie) => {
                    acc[cookie.name] = cookie;
                    return acc;
                },
                {} as Record<string, Cookie>
            );
    }

    public async text(): Promise<string> {
        const buffer = await this._response.body();
        const decoder = new TextDecoder('utf-8');
        return decoder.decode(buffer);
    }

    public async json(): Promise<object> {
        return await this._response.json();
    }

    public async parse<T extends z.AnyZodObject>(schema: T): Promise<z.infer<T>> {
        const data = await this._response.json();
        try {
            return schema.strict().parse(data);
        } catch (err) {
            console.error('Failed to parse response', data, 'with error', err);
            const error = err as z.ZodError;
            throw new Error(error.message);
        }
    }

    public async parseProblem(): Promise<Problem> {
        return await this.parse(ProblemSchema);
    }
}

export class ApiRequest<Q = void> {
    public method: ApiMethod;
    public url: string;
    public headers: Record<string, string | undefined>;
    public params: ApiParams;
    public body?: Payload<Q>;

    constructor(
        method: ApiMethod,
        url: string,
        headers: Record<string, string> = {},
        params: ApiParams = {},
        body: Payload<Q> | undefined = undefined
    ) {
        this.method = method;
        this.url = url;
        this.headers = headers;
        this.params = params;
        this.body = body;
    }

    public static get<Q = void>(url: string): ApiRequest<Q> {
        return new ApiRequest<Q>('get', url);
    }

    public static post<Q = void>(url: string, body?: Payload<Q>): ApiRequest<Q> {
        return new ApiRequest<Q>('post', url).withBody(body);
    }

    public static put<Q = void>(url: string, body?: Payload<Q>): ApiRequest<Q> {
        return new ApiRequest<Q>('put', url).withBody(body);
    }

    public static patch<Q = void>(url: string, body?: Payload<Q>): ApiRequest<Q> {
        return new ApiRequest<Q>('patch', url).withBody(body);
    }

    public static delete<Q = void>(url: string): ApiRequest<Q> {
        return new ApiRequest<Q>('delete', url);
    }

    withHeaders(headers: Record<string, string | undefined>): ApiRequest<Q> {
        this.headers = { ...this.headers, ...convertKeysToLowerCase(headers) };
        return this;
    }

    withAuthIf(token: string | undefined | null): ApiRequest<Q> {
        if (token) {
            return this.withHeaders({ authorization: `Bearer ${token}` });
        }
        return this;
    }

    withAuth(token: string): ApiRequest<Q> {
        return this.withHeaders({ authorization: `Bearer ${token}` });
    }

    withCookies(list: Record<string, string>): ApiRequest<Q> {
        const cookies = Object.entries(list)
            .map(([key, value]) => `${key}=${value}`)
            .join(';');
        return this.withHeaders({ cookie: cookies });
    }

    withParams(params: ApiParams): ApiRequest<Q> {
        this.params = { ...this.params, ...params };
        return this;
    }

    withBody(data: Payload<Q> | undefined): ApiRequest<Q> {
        this.body = data;
        return this;
    }

    private bodyJson(): string | undefined {
        if (this.body === undefined) {
            return undefined;
        }
        if ((this.body as Unstructured).type === 'unstructured') {
            return JSON.stringify((this.body as Unstructured).rawData);
        }
        return JSON.stringify(this.body);
    }

    private async send(): Promise<ApiResponse> {
        const context = await request.newContext();

        const log_id = randomUUID();
        const headers = removeUndefinedValues(this.headers);

        const data = this.bodyJson();
        if (data !== undefined) {
            headers['content-type'] = 'application/json';
        }

        log(
            `Request [${log_id}] ${this.method} ${this.url}\nparams: ${JSON.stringify(this.params, null, 2)}\nheaders: ${JSON.stringify(headers, null, 2)}`
        );
        if (data !== undefined) {
            log(`Request body [${log_id}]: ${data}`);
        }

        let response;
        switch (this.method) {
            case 'get': {
                response = await context.get(this.url, { headers, params: this.params, data });
                break;
            }
            case 'post': {
                response = await context.post(this.url, { headers, params: this.params, data });
                break;
            }
            case 'put': {
                response = await context.put(this.url, { headers, params: this.params, data });
                break;
            }
            case 'patch': {
                response = await context.patch(this.url, { headers, params: this.params, data });
                break;
            }
            case 'delete': {
                response = await context.delete(this.url, { headers, params: this.params, data });
                break;
            }
        }

        const api_response = new ApiResponse(context, response);

        // todo: it may effect the test as we pre-await properties before the actual test
        const response_headers = api_response.headers();
        const response_cookies = api_response.cookies();
        const response_text = await api_response.text();
        log(
            `Response [${log_id}] ${api_response.status()}\nheaders: ${JSON.stringify(response_headers, null, 2)}\ncookies: ${JSON.stringify(response_cookies, null, 2)}`
        );
        if (response_text) {
            log(`Response body [${log_id}]:\n${response_text}`);
        }

        return api_response;
    }

    then<TResult1 = ApiResponse, TResult2 = never>(
        onfulfilled?: ((value: ApiResponse) => TResult1 | PromiseLike<TResult1>) | null,
        onrejected?: ((reason: void) => TResult2 | PromiseLike<TResult2>) | null
    ): Promise<TResult1 | TResult2> {
        return this.send().then(onfulfilled, onrejected);
    }
}
