import js from '@eslint/js';
import typescriptEslint from '@typescript-eslint/eslint-plugin';
import prettier from 'eslint-config-prettier';
import ts from 'typescript-eslint';

export default [
    js.configs.recommended,
    ...ts.configs.recommended,
    prettier,
    {
        plugins: {
            '@typescript-eslint': typescriptEslint
        },

        languageOptions: {
            parser: ts.parser,
            ecmaVersion: 5,
            sourceType: 'script',

            parserOptions: {
                project: './tsconfig.json'
            }
        },

        rules: {
            '@typescript-eslint/no-unused-vars': [
                'error',
                {
                    args: 'all',
                    argsIgnorePattern: '^_',
                    caughtErrors: 'all',
                    caughtErrorsIgnorePattern: '^_',
                    destructuredArrayIgnorePattern: '^_',
                    varsIgnorePattern: '^_',
                    ignoreRestSiblings: true
                }
            ],
            //'@typescript-eslint/no-unused-vars': ['error', { argsIgnorePattern: '^_|_' }],
            '@typescript-eslint/no-floating-promises': ['error']
        }
    },
    {
        ignores: ['node_modules/', 'reports/', 'todo/']
    }
];
