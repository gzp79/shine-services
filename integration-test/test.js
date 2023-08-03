#! /usr/bin/env node
import karate from '@karatelabs/karate';
import dockerCompose from 'docker-compose';

async function main() {
    const dockerOptions = {
        config: '../docker-integration-test.yml'
    };

    try {
        console.log('Starting docker...');
        await dockerCompose.upAll({
            commandOptions: ['--build'],
            ...dockerOptions
        });

        console.log('Running karate tests...');
        karate.exec();
    } finally {
        console.log('Stopping docker...');
        await dockerCompose.down({
            ...dockerOptions
        });
    }
}

await main();
