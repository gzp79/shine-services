import js from '@eslint/js';
import stylistic from '@stylistic/eslint-plugin';
import prettier from 'eslint-config-prettier';
import ts from 'typescript-eslint';

export default [
    js.configs.recommended,
    ...ts.configs.recommended,
    prettier,
    {
        plugins: {
            '@stylistic': stylistic
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
            '@stylistic/quotes': ['error', 'single'],
            '@typescript-eslint/no-floating-promises': ['error']
        }
    },
    {
        ignores: ['node_modules/', 'reports/', 'todo/']
    }
];
