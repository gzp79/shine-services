const { pathsToModuleNameMapper } = require('ts-jest');
const { compilerOptions } = require('./tsconfig.json');

const config = {
    testMatch: ['**/regression/*.ts', '**/regression/**/*.ts'],
    transform: {
        '^.+\\.(ts|tsx)$': 'ts-jest'
    },    
    moduleNameMapper: pathsToModuleNameMapper(compilerOptions.paths, {prefix: "<rootDir>"}),    
};

module.exports = config;
