export function intoMatcherResult(self: jest.MatcherContext, received: any, expected: object) {
    const pass = self.equals(received, expected);

    if (pass) {
        return {
            message: () =>
                `Expected: ${self.utils.printExpected(expected)}\nReceived: ${self.utils.printReceived(
                    received
                )}`,
            pass: true
        };
    }
    return {
        message: () =>
            `Expected: ${self.utils.printExpected(expected)}\nReceived: ${self.utils.printReceived(
                received
            )}\n\n${self.utils.diff(expected, received, {})}`,
        pass: false
    };
}
