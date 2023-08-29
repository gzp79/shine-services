const config = [
    '--require-module ts-node/register',
    '--require-module tsconfig-paths/register',
    '--require karate/**/*.ts',
    '--require regression/**/*.ts',
    '--format @cucumber/pretty-formatter',
    '--format html:reports/cucumber_report.html'
].join(' ');

module.exports = {
    default: config
};
