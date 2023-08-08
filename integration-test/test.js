#! /usr/bin/env node
import karate from '@karatelabs/karate';
import shell from 'shelljs';
import dockerCompose from 'docker-compose';

const dockerOptions = {
    config: '../docker-integration-test.yml'
};

const mocks = ['mocking/openid.feature'];

async function setup() {
    // start up mock server required for service initialization
    let sh = await new Promise((resolve, _reject) => {
        console.log('Starting mock server...');
        const args = '-p 8080 -m ' + mocks.join(',');

        // some ugly hack to start karate in the background using jbang
        process.env['KARATE_META'] = 'npm:' + process.env.npm_package_version;
        const argLine =
            karate.jvm.args + ' ' + karate.executable() + ' ' + args;
        const sh = shell.exec('jbang ' + argLine, { async: true });
        sh.stdout.on('data', (data) => {
            if (data.includes('server started')) {
                resolve(sh);
            }
        });
    });

    console.log('Starting service...');
    await dockerCompose.upAll({
        commandOptions: ['--build'],
        ...dockerOptions
    });

    console.log('Stopping mock server...');
    try {
        await fetch('http://localhost:8080/stop');
    } catch {
        /* it should be an ECONNRESET as karate has been stopped */
    }
    console.log('Setup completed.');
}

async function tearDown() {
    console.log('Stopping docker...');
    await dockerCompose.down({
        ...dockerOptions
    });
    console.log('Tear down completed.');
}

async function main() {
    try {
        console.log('Running setup...');
        await setup();
        console.log('Running karate tests...');
        karate.exec();
    } finally {
        console.log('Running tear down...');
        await tearDown();
    }
}

await main();
