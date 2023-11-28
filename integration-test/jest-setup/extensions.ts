import * as matchers from 'jest-extended';
import debug from 'debug';
import process from 'process';
import 'jest-expect-message';
import authExts from '$lib/expect/auth_exts';
import userExts from '$lib/expect/user_exts';

expect.extend(matchers);
expect.extend(authExts);
expect.extend(userExts);
//expect.extend(message);

// Allow the usage of self signed certificates
process.env.NODE_TLS_REJECT_UNAUTHORIZED = '0';

// allow superagent logging
debug.enable('superagent');
debug.log = console.log.bind(console);
