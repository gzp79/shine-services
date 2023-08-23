import { Given } from '@cucumber/cucumber';

export const appDomain = 'scytta-test.com';
export const serviceDomain = 'cloud.' + appDomain;
export const serviceUrl = 'http://' + serviceDomain;
export const identityUrl = serviceUrl + '/identity';

declare global {
    interface String {
        format(kv: Record<string, string>): string;
    }
}

if (!String.prototype.format) {
    String.prototype.format = function (kv) {
        return this.replace(/\${([_a-zA-Z][_a-zA-Z0-9]*)}/, function (_match, key) {
            return kv[key] === undefined ? `{${key}:undefined}` : kv[key];
        });
    };
}
