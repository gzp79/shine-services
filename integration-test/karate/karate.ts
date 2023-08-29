import { CucumberAttachments, CucumberLog, binding } from 'cucumber-tsflow';
import _chai from './_chai';
import { KarateState } from './_karate_state';
import { World } from '@cucumber/cucumber';

export interface KarateWorld extends World {
    [key: string]: any;
}

export interface KarateLogger {
    log(message: any): void;
    logAttach(data: any, mime: string): void;
}

export class KarateCore implements KarateLogger {
    public constructor(
        private readonly logger: CucumberLog,
        private readonly logAttachments: CucumberAttachments,
        protected readonly karate: KarateState
    ) {}

    public log(message: any) {
        console.log(message);
        this.logger.log(message);
    }

    public logAttach(data: any, mime: string) {
        this.logAttachments.attach(data, mime);
    }
}

export { KarateState } from './_karate_state';
export { HttpMethod } from './_karate_types';
export { MockServer, TypedResponse, TypedRequest } from './_mock_server';
export const chai = _chai;
export const expect = chai.expect;
export const assert = chai.assert;
export const request = chai.request;
