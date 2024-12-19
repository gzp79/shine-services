const { pathsToModuleNameMapper } = require('ts-jest');
const { compilerOptions } = require('./tsconfig.json');

const config = {
    testMatch: ['**/regression/*.ts', '**/regression/**/*.ts'],
    transform: {
        '^.+\\.(ts|tsx)$': 'ts-jest'
    },
    moduleNameMapper: pathsToModuleNameMapper(compilerOptions.paths, { prefix: '<rootDir>' }),
    setupFilesAfterEnv: ['<rootDir>/jest-setup/extensions.ts'],
     reporters: [         
        [
            'jest-junit',
            {
                includeConsoleOutput: true,
                reportTestSuiteErrors: true,
                outputDirectory: './reports',
                usePathForSuiteName: true
            }
        ],
        'default'
    ]
};

module.exports = config;
