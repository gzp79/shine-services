interface CustomMatchers<R = unknown> {
    toBeEarlier(dateToCompare?: Partial<Date>): R;
    toBeLater(dateToCompare?: Partial<Date>): R;
}

declare global {
    namespace jest {
        interface Expect extends CustomMatchers {}
        interface Matchers<R> extends CustomMatchers<R> {}
        interface InverseAsymmetricMatchers extends CustomMatchers {}
    }
}

expect.extend({
    toBeEarlier(received: Date, dateToCompare: Date) {
        const pass = received.getTime() < dateToCompare.getTime();
        if (pass) {
            return {
                message: () =>
                    `Expected ${received.toISOString()} not to be before ${dateToCompare.toISOString()}`,
                pass: true
            };
        } else {
            return {
                message: () =>
                    `Expected ${received.toISOString()} to be before ${dateToCompare.toISOString()}`,
                pass: false
            };
        }
    },

    toBeLater(received: Date, dateToCompare: Date) {
        const pass = received.getTime() > dateToCompare.getTime();
        if (pass) {
            return {
                message: () =>
                    `Expected ${received.toISOString()} not to be before ${dateToCompare.toISOString()}`,
                pass: true
            };
        } else {
            return {
                message: () =>
                    `Expected ${received.toISOString()} to be before ${dateToCompare.toISOString()}`,
                pass: false
            };
        }
    }
});

export {};
