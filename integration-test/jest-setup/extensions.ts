import * as matchers from 'jest-extended';
import debug from 'debug';
import process from 'process';
import 'jest-expect-message';

expect.extend(matchers);
//expect.extend(message);

// Allow the usage of self signed certificates
process.env.NODE_TLS_REJECT_UNAUTHORIZED = '0';

// allow superagent logging
debug.enable('superagent');
debug.log = console.log.bind(console);
