import { expect as baseExpect } from '@playwright/test';
import { ParsedMail } from 'mailparser';

export const expect = baseExpect.extend({
    toHaveSingleTo(actual: ParsedMail, address: string) {
        const pass = !Array.isArray(actual.to) && actual.to?.text === address;

        if (pass) {
            return {
                message: () => `expected ${actual.to} not to be [${address}]`,
                pass: true
            };
        } else {
            return {
                message: () => `expected ${actual.to} to be [${address}]`,
                pass: false
            };
        }
    }
});
