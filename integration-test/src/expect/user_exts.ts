import uuidValidate from 'uuid-validate';
import { intoMatcherResult } from './utils';
import { UserInfo } from '$lib/api/user_api';

interface CustomMatchers<R = unknown> {
    toBeGuestUser(): R;
}

declare global {
    namespace jest {
        interface Expect extends CustomMatchers {}
        interface Matchers<R> extends CustomMatchers<R> {}
        interface InverseAsymmetricMatchers extends CustomMatchers {}
    }
}

const matchers: jest.ExpectExtendMap = {
    toBeGuestUser(received: UserInfo) {
        const expected = expect.objectContaining({
            userId: expect.toSatisfy((id: any) => uuidValidate(id)),
            name: expect.toStartWith('Freshman_'),
            sessionLength: expect.not.toBeNegative(),
            roles: []
        });
        return intoMatcherResult(this, received, expected);
    }
};

export default matchers;
