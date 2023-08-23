const config = [
    'regression/**/*.feature',
    '--require-module ts-node/register',
    '--require-module tsconfig-paths/register',
    '--require steps/**/*.ts',
    '--format @cucumber/pretty-formatter',
    '--format html:reports/cucumber_report.html',
].join(' ');

module.exports = {
    default: config
};
