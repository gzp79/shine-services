#! /usr/bin/env node
import karate from '@karatelabs/karate';
import shell from 'shelljs';
import dockerCompose from 'docker-compose';

const dockerOptions = {
    config: '../docker.yml',
    log: true
};

async function setup() {
    console.log('Starting service...');
    await dockerCompose.upAll({
        commandOptions: ['--build'],
        ...dockerOptions
    });
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
    console.log('Bye.');
}

await main();
