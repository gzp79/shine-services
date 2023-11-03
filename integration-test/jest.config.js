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
        'default',
        [
            'jest-stare',
            {
                resultDir: './reports',
                reportTitle: 'Test Report',
                reportSummary: true,
                additionalResultsProcessors: ['jest-junit'],
                log: true
            }
        ]
    ]
};

module.exports = config;
