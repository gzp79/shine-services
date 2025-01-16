import { expect } from '@playwright/test';

export function intoMatcherResult(received: any, expected: object) {
    try {
        expect(received).toEqual(expected);
        return {
            message: () => `Expected: ${expected}\nReceived: ${received}`,
            pass: true
        };
    } catch (error) {
        return {
            message: () => `Expected: ${expected}\nReceived: ${received}\n\n${diff(expected, received)}`,
            pass: false
        };
    }
}

function diff(expected: object, received: any) {
    // Implement a diff function or use a library to show the difference
    return JSON.stringify(expected) !== JSON.stringify(received) ? 'Objects are different' : '';
}
