import js from '@eslint/js';
import prettier from 'eslint-config-prettier';
import globals from 'globals';
import ts from 'typescript-eslint';

/** @type {import('eslint').Linter.Config[]} */
export default [
    js.configs.recommended,
    ...ts.configs.recommended,
    prettier,
    {
        languageOptions: {
            globals: {
                ENABLE_MOCK: 'readonly',
                ...globals.browser,
                ...globals.node
            }
        }
    },
    {
        languageOptions: {
            parser: ts.parser
        },
        rules: {
            '@typescript-eslint/no-unused-vars': ['error', { argsIgnorePattern: '^_|_' }]
        }
    },
    {
        ignores: ['build/']
    }
];
