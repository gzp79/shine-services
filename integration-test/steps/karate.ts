import { CucumberLog, binding } from 'cucumber-tsflow';
import _chai from './_chai';
import { KarateState } from './_karate_state';

export class KarateCore {
    public constructor(
        private readonly logger: CucumberLog,
        protected readonly karate: KarateState
    ) {}

    public log(message: string, ...detail: any[]) {
        const msg = `${message} with details: ${JSON.stringify(detail)}`;
        console.log(message);
        this.logger.log(message);
    }
}

export { Config } from './_config';
export { KarateState } from './_karate_state';
export { HttpMethod } from './_karate_types';
export const chai = _chai;
export const expect = chai.expect;
export const assert = chai.assert;
export const request = chai.request;
