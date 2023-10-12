declare global {
    interface String {
        format(kv: Record<string, string>): string;
        formatExpr(context: any): string;

        urlDecode(): string;
        urlEncode(): string;
        parseAsQueryParams(): Record<string, string>;
        parseQueryParamsFromUrl(): Record<string, string>;
    }
}

if (!String.prototype.format) {
    String.prototype.format = function (kv) {
        return this.replace(/\${([_a-zA-Z][_a-zA-Z0-9]*)}/, function (_match, key) {
            return kv[key] ?? `{UNDEFINED<${key}>}`;
        });
    };

    String.prototype.formatExpr = function (context) {
        return this.replace(/\${(.*)}/, function (_match, expr) {
            const value = new Function('karate', `return ${expr}`)(context);
            return value ?? `{UNDEFINED<${expr}>}`;
        });
    };

    String.prototype.urlEncode = function () {
        return encodeURIComponent(this as string);
    };

    String.prototype.urlDecode = function () {
        return decodeURIComponent(this as string);
    };

    String.prototype.parseAsQueryParams = function (): Record<string, string> {
        let o: Record<string, string> = {};
        this.split('&')
            .map((x) => x.split('='))
            .forEach((x) => (o[x[0]] = x[1]?.urlDecode()));
        return o;
    };

    String.prototype.parseQueryParamsFromUrl = function (): Record<string, string> {
        return this.split('?')[1].parseAsQueryParams();
    };
}

export function createUrlQueryString(params: Record<string, String>): string {
    const p = [];
    for (const k in params) {
        p.push(`${k}=${params[k].urlEncode()}`);
    }
    return p.join('&');
}

export function generateRandomString(len: number, alpha?: string) {
    const characters = alpha ?? 'ABCDEFGHIJKLMNOPQRSTUVWXYZ';
    let result = '';
    for (let i = 0; i < len; i++) {
        result += characters.charAt(Math.floor(Math.random() * characters.length));
    }
    return result;
}
