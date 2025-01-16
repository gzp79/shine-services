import { expect as baseExpect } from '@playwright/test';
import { APIResponse } from 'playwright';

export const expect = baseExpect.extend({
    toHaveStatus(actual: APIResponse, statusCode) {
        const pass = actual.status() === statusCode;
        if (pass) {
            return {
                message: () => `expected ${actual.status()} to be ${statusCode}`,
                pass: true
            };
        } else {
            return {
                message: () => `unexpected status(${actual.status()}): ${actual.text}`,
                pass: false
            };
        }
    },

    toHaveHeader(actual: APIResponse, header: string, value: string | undefined) {
        const headerValue = actual.headers()[header];

        const pass = actual.headers()[header] === value;
        if (pass) {
            return {
                message: () => `expected ${headerValue} to be ${value}`,
                pass: true
            };
        } else {
            return {
                message: () => `expected ${headerValue} not to be ${value}`,
                pass: false
            };
        }
    }
});
