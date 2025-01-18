import { expect as baseExpect } from '@playwright/test';

export const expect = baseExpect.extend({
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
