import request, { Request as Req } from 'superagent';

export default class WrappedRequest {
    static get(url: string): Req {
        return request.get(url).ok((res) => true);
    }

    static post(url: string): Req {
        return request.post(url).ok((res) => true);
    }

    static put(url: string): Req {
        return request.put(url).ok((res) => true);
    }

    static delete(url: string): Req {
        return request.delete(url).ok((res) => true);
    }
}

export type { Response, Request } from 'superagent';
