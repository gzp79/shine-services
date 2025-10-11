import { expect, test } from '$fixtures/setup';
import { ApiRequest } from '$lib/api/api';
import { StaticFileServer } from '$lib/mocks/static_file_server';
import fs from 'fs';
import path from 'path';

test.describe('Static File Server', () => {
    const testDir = path.join(__dirname, '..', 'node_modules', '.tmp');
    const testFile = path.join(testDir, 'test.txt');
    const testContent = 'Hello, World!';

    test.beforeAll(async () => {
        if (!fs.existsSync(testDir)) {
            fs.mkdirSync(testDir, { recursive: true });
        }
        fs.writeFileSync(testFile, testContent);
        console.log(`Test file created at: ${testFile}`);
    });

    // test.afterAll(async () => {
    //     if (fs.existsSync(testFile)) {
    //         fs.unlinkSync(testFile);
    //     }
    // });

    //todo: for some reason tls fails here (but works from the tools)
    test.skip('should serve static files with security headers', async () => {
        const mock = new StaticFileServer('static-server', {
            url: new URL('https://local.scytta.com:9080'),
            staticFilesPath: testDir
        });

        await mock.start();
        expect(mock.isRunning).toBeTruthy();

        // Test file serving
        const response = await ApiRequest.get(mock.getUrlFor('/test.txt'));
        expect(response).toHaveStatus(200);
        expect(await response.text()).toBe(testContent);

        // Test security headers
        const headers = response.headers();
        expect(headers['x-frame-options']).toBe('DENY');
        expect(headers['x-content-type-options']).toBe('nosniff');
        expect(headers['referrer-policy']).toBe('no-referrer');
        expect(headers['permissions-policy']).toBe('document-domain=()');
        expect(headers['content-security-policy']).toContain('worker-src "none"');

        await mock.stop();
        expect(mock.isRunning).toBeFalsy();
    });
});
