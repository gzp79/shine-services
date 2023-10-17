import * as matchers from 'jest-extended';
import * as process from 'process';
import 'jest-expect-message';

expect.extend(matchers);
//expect.extend(message);

// allow the usage of self signed certificatess
process.env['NODE_TLS_REJECT_UNAUTHORIZED'] = '0';
