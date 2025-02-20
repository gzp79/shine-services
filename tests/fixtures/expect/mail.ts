import { expect as baseExpect } from '@playwright/test';
import { ParsedMail } from 'mailparser';

export const expect = baseExpect.extend({
    toHaveMailTo(actual: ParsedMail, address: string | string[]) {
        let toList: string[] = [];
        if (Array.isArray(actual.to)) {
            toList = actual.to.map((to) => to.text);
        } else if (actual.to) {
            toList = [actual.to.text];
        }
        if (typeof address === 'string') {
            address = [address];
        }

        const pass = address.every((addr) => toList.includes(addr)) && toList.length === address.length;

        if (pass) {
            return {
                message: () => `expected [${toList.join(', ')}] not to be [${address}]`,
                pass: true
            };
        } else {
            return {
                message: () => `expected [${toList.join(', ')}] to be [${address}]`,
                pass: false
            };
        }
    },

    toHaveMailFrom(actual: ParsedMail, address: string) {
        const pass = actual.from?.text === address;

        if (pass) {
            return {
                message: () => `expected [${actual.from?.text}] not to be [${address}]`,
                pass: true
            };
        } else {
            return {
                message: () => `expected [${actual.from?.text}] to be [${address}]`,
                pass: false
            };
        }
    }
});
