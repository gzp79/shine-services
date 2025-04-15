import { expect as baseExpect } from '@playwright/test';

function toHaveType(received: unknown, expected: string) {
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
    //toHaveType,
    toBeString(received: unknown) {
        return toHaveType(received, 'string');
    },
    toBeNumber(received: unknown) {
        return toHaveType(received, 'number');
    },
    toBeBoolean(received: unknown) {
        return toHaveType(received, 'boolean');
    },
    toBeObject(received: unknown) {
        return toHaveType(received, 'object');
    },
    toBeArray(received: unknown) {
        return toHaveType(received, 'array');
    },
    toBeFunction(received: unknown) {
        return toHaveType(received, 'function');
    },

    toBeEmptyValue(received: unknown) {
        let pass;
        let formatted = (received as any).toString();
        if (received === null || received === undefined) {
            pass = true;
        } else if (typeof received === 'string') {
            pass = received === '';
        } else if (Array.isArray(received)) {
            pass = received.length === 0;
            formatted = JSON.stringify(received);
        } else {
            throw new Error(`toBeEmptyValue: unsupported type ${typeof received}`);
        }

        if (pass) {
            return {
                message: () => `expected ${formatted} not to be empty`,
                pass: true
            };
        } else {
            return {
                message: () => `expected ${formatted} to be empty`,
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
