import js from '@eslint/js';
import stylisticTs from '@stylistic/eslint-plugin-ts';
import prettier from 'eslint-config-prettier';
import ts from 'typescript-eslint';

export default [
    js.configs.recommended,
    ...ts.configs.recommended,
    prettier,
    {
        plugins: {
            '@stylistic/ts': stylisticTs
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
            '@typescript-eslint/no-floating-promises': ['error'],
            '@stylistic/ts/quotes': ['error', 'single']
        }
    },
    {
        ignores: ['node_modules/', 'reports/', 'todo/']
    }
];
