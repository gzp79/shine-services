const { pathsToModuleNameMapper } = require('ts-jest');
const { compilerOptions } = require('./tsconfig.json');

const config = {
    testMatch: ['**/regression/*.ts', '**/regression/**/*.ts'],
    transform: {
        '^.+\\.(ts|tsx)$': 'ts-jest'
    },
    moduleNameMapper: pathsToModuleNameMapper(compilerOptions.paths, { prefix: '<rootDir>' }),
    reporters: [
        'default',
        [
            'jest-html-reporters',
            {
                publicPath: './reports/html-report',
                filename: 'report.html',
                openReport: false,
                includeConsoleLog: true,
            }
        ]
    ]
};

module.exports = config;
