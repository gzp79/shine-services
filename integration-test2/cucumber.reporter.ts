// generate cucumber html report from the json
import { Options, generate } from 'cucumber-html-reporter';

const options: Options = {
    theme: 'bootstrap',
    jsonFile: 'reports/cucumber_report.json',
    output: 'reports/cucumber_report.html',
    reportSuiteAsScenarios: true,
    scenarioTimestamp: true,
    launchReport: true    
};

generate(options);
