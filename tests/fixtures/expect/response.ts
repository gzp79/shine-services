import { MatcherReturnType, expect as baseExpect } from '@playwright/test';
import { ApiResponse } from '$lib/api/api';

export const expect = baseExpect.extend({
    toHaveStatus(actual: ApiResponse, statusCode: number): MatcherReturnType {
        const status = actual.status();
        const pass = status === statusCode;
        if (pass) {
            return {
                message: () => `expected ${status} not to be ${statusCode}`,
                pass: true
            };
        } else {
            return {
                message: () => `expected ${status} to be ${statusCode}`,
                pass: false
            };
        }
    },

    toHaveHeader(actual: ApiResponse, header: string, value: undefined | string | string[]): MatcherReturnType {
        const headers = actual.headers();
        const headerValue = headers[header];

        let pass = false;
        if (value === undefined) {
            pass = headerValue === undefined;
        } else if (typeof value === 'string') {
            pass = headerValue === value;
        } else {
            if (Array.isArray(headerValue)) {
                const sortedValue = value.sort();
                const sortedHeaderValue = headerValue.sort();
                pass =
                    headerValue.length === value.length &&
                    sortedHeaderValue.every((val, index) => val === sortedValue[index]);
            }
        }

        if (pass) {
            return {
                message: () => `expected ${headerValue} not to be ${value}`,
                pass: true
            };
        } else {
            return {
                message: () => `expected ${headerValue} to be ${value}`,
                pass: false
            };
        }
    }
});
