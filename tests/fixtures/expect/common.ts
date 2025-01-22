/* eslint-disable @typescript-eslint/no-explicit-any */
import { expect as baseExpect } from '@playwright/test';

function toHaveType(received: any, expected: string) {
    const pass = typeof received === expected;
    if (pass) {
        return {
            message: () => `expected ${received} not to have type ${expected}`,
            pass: true
        };
    } else {
        return {
            message: () => `expected ${received} to have type ${expected}`,
            pass: false
        };
    }
}

export const expect = baseExpect.extend({
    toHaveType,
    toBeString(received: any) {
        return toHaveType(received, 'string');
    },
    toBeNumber(received: any) {
        return toHaveType(received, 'number');
    },
    toBeBoolean(received: any) {
        return toHaveType(received, 'boolean');
    },
    toBeObject(received: any) {
        return toHaveType(received, 'object');
    },
    toBeArray(received: any) {
        return toHaveType(received, 'array');
    },
    toBeFunction(received: any) {
        return toHaveType(received, 'function');
    },

    toBeEmpty(received: any) {
        let pass;
        if (received === null || received === undefined) {
            pass = true;
        } else if (typeof received === 'string') {
            pass = received === '';
        } else if (Array.isArray(received)) {
            pass = received.length === 0;
        } else {
            throw new Error(`toBeEmpty: unsupported type ${typeof received}`);
        }

        if (pass) {
            return {
                message: () => `expected ${received} not to be empty`,
                pass: true
            };
        } else {
            return {
                message: () => `expected ${received} to be empty`,
                pass: false
            };
        }
    },

    toBeBefore(received: Date | number, expected: Date | number) {
        if (typeof received === 'number') {
            received = new Date(received);
        }

        if (typeof expected === 'number') {
            expected = new Date(expected);
        }

        const pass = received < expected;
        if (pass) {
            return {
                message: () => `expected ${received} not to be before ${expected}`,
                pass: true
            };
        } else {
            return {
                message: () => `expected ${received} to be before ${expected}`,
                pass: false
            };
        }
    },
    toBeAfter(received: Date, expected: Date) {
        if (typeof received === 'number') {
            received = new Date(received);
        }

        if (typeof expected === 'number') {
            expected = new Date(expected);
        }

        const pass = received > expected;
        if (pass) {
            return {
                message: () => `expected ${received} not to be before ${expected}`,
                pass: true
            };
        } else {
            return {
                message: () => `expected ${received} to be before ${expected}`,
                pass: false
            };
        }
    },

    toStartWith(received: string, expected: string) {
        const pass = received.startsWith(expected);
        if (pass) {
            return {
                message: () => `expected ${received} not to start with ${expected}`,
                pass: true
            };
        } else {
            return {
                message: () => `expected ${received} to start with ${expected}`,
                pass: false
            };
        }
    },
    toEndWith(received: string, expected: string) {
        const pass = received.endsWith(expected);
        if (pass) {
            return {
                message: () => `expected ${received} not to end with ${expected}`,
                pass: true
            };
        } else {
            return {
                message: () => `expected ${received} to end with ${expected}`,
                pass: false
            };
        }
    }
});
