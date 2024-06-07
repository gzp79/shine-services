import request from 'superagent';

declare global {
    namespace jest {
        interface Matchers<R, T = {}> {
            toHaveStatus(statusCode: number): R;
        }
    }
}

const matchers: jest.ExpectExtendMap = {
    toHaveStatus(received: any, statusCode: number) {
        if (
            typeof statusCode !== 'number' ||
            typeof received !== 'object' ||
            !(received as request.Response).statusCode
        ) {
            throw new TypeError('Incorrect types provided!');
        }
        const response = received as request.Response;
        const pass = (response as request.Response).statusCode === statusCode;

        const receivedMsg = this.utils.printReceived(response.statusCode);
        const expectedMsg = this.utils.printExpected(response.statusCode);
        const body = response.text ?? '';
        const negate = pass ? ' not' : '';

        return {
            message: () =>
                `expected status code ${receivedMsg} ${negate}to be ${expectedMsg}\n response: ${body}}`,
            pass: pass
        };
    }
};

export default matchers;
