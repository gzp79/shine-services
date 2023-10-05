const config = {
    testMatch: [
        '**/regression/*.ts',
        '**/regression/**/*.ts'
    ],
    transform: {
        '^.+\\.(ts|tsx)$': 'ts-jest'
    }
};

module.exports = config;
