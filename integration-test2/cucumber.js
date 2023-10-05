const config = [
    '--require-module ts-node/register',
    '--require-module tsconfig-paths/register',
    '--require karate/**/*.ts',
    '--require regression/**/*.ts',
    '--format @cucumber/pretty-formatter',
    '--format html:reports/cucumber_report_simple.html',
    '--format json:reports/cucumber_report.json'
].join(' ');

module.exports = {
    default: config
};
