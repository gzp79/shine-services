import { DEFAULT_USER_AGENT } from '$lib/utils';
import { createHash, createHmac, randomBytes, randomUUID } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { createClient } from 'redis';

type ServerTestConfig = {
    service: {
        sessionSecret: string;
        sessionTtl: number;
        sessionRedisCns: string;
    };
};

type CreateUserSessionInput = {
    userId?: string;
    name?: string;
    isEmailConfirmed?: boolean;
    isLinked?: boolean;
    roles?: string[];
    userAgent?: string;
};

type UpdateUserInput = {
    name?: string;
    isEmailConfirmed?: boolean;
    isLinked?: boolean;
    roles?: string[];
    userAgent?: string;
};

export type MintedSession = {
    sessionCookie: string;
    userId: string;
    sessionKeyHex: string;
    fingerprint: string;
};

export class SessionMint {
    private readonly signingKey: Buffer;
    private readonly createdSessions = new Set<string>();

    private constructor(
        private readonly redisUrl: string,
        private readonly sessionTtlSeconds: number,
        sessionSecretBase64UrlNoPad: string
    ) {
        const masterKey = decodeBase64UrlNoPad(sessionSecretBase64UrlNoPad);
        if (masterKey.length < 32) {
            throw new Error(`sessionSecret is too short: expected >= 32 bytes, got ${masterKey.length}`);
        }

        // Rust cookie::Key::signing() uses the first 32 bytes for HMAC-SHA256 signing.
        this.signingKey = masterKey.subarray(0, 32);
    }

    public static async fromServerConfig(
        configPath = path.join(__dirname, '../../../services/server_config_test.json')
    ): Promise<SessionMint> {
        const content = await readFile(configPath, 'utf-8');
        const config = JSON.parse(content) as ServerTestConfig;
        const service = config.service;
        return new SessionMint(service.sessionRedisCns, service.sessionTtl, service.sessionSecret);
    }

    public async createUserSession(input: CreateUserSessionInput = {}): Promise<MintedSession> {
        const userId = input.userId ?? randomUUID();
        const sessionKeyHex = randomBytes(16).toString('hex');
        const fingerprint = fingerprintFromUserAgent(input.userAgent ?? DEFAULT_USER_AGENT);

        const userData = {
            name: input.name ?? `Minted_${userId.slice(0, 8)}`,
            isEmailConfirmed: input.isEmailConfirmed ?? false,
            isLinked: input.isLinked ?? false,
            roles: input.roles ?? []
        };

        const keys = sessionRedisKeys(userId, sessionKeyHex);
        const sentinel = {
            createdAt: new Date().toISOString(),
            fingerprint
        };

        await this.withRedis(async (redis) => {
            await redis.set(keys.sentinelKey, JSON.stringify(sentinel), { EX: this.sessionTtlSeconds });
            await redis.set(keys.dataKey, JSON.stringify(userData), { EX: this.sessionTtlSeconds });
        });
        this.createdSessions.add(this.sessionIdentity(userId, sessionKeyHex));

        const sidPayload = {
            u: userId,
            key: sessionKeyHex,
            fp: fingerprint
        };

        return {
            sessionCookie: this.signSidPayload(sidPayload),
            userId,
            sessionKeyHex,
            fingerprint
        };
    }

    public async updateUser(session: MintedSession, input: UpdateUserInput = {}): Promise<MintedSession> {
        const keys = sessionRedisKeys(session.userId, session.sessionKeyHex);

        await this.withRedis(async (redis) => {
            const rawData = await redis.get(keys.dataKey);
            if (!rawData) {
                throw new Error('Cannot update session user: session data not found in Redis');
            }

            const data = JSON.parse(rawData) as {
                name: string;
                isEmailConfirmed: boolean;
                isLinked: boolean;
                roles: string[];
            };

            const nextData = {
                name: input.name ?? data.name,
                isEmailConfirmed: input.isEmailConfirmed ?? data.isEmailConfirmed,
                isLinked: input.isLinked ?? data.isLinked,
                roles: input.roles ?? data.roles
            };

            await redis.set(keys.dataKey, JSON.stringify(nextData), { EX: this.sessionTtlSeconds });

            if (input.userAgent !== undefined) {
                const rawSentinel = await redis.get(keys.sentinelKey);
                if (!rawSentinel) {
                    throw new Error('Cannot update session user: session sentinel not found in Redis');
                }
                const sentinel = JSON.parse(rawSentinel) as { createdAt: string; fingerprint: string };
                sentinel.fingerprint = fingerprintFromUserAgent(input.userAgent);
                await redis.set(keys.sentinelKey, JSON.stringify(sentinel), { EX: this.sessionTtlSeconds });
            }
        });

        const nextFingerprint =
            input.userAgent !== undefined ? fingerprintFromUserAgent(input.userAgent) : session.fingerprint;

        const sidPayload = {
            u: session.userId,
            key: session.sessionKeyHex,
            fp: nextFingerprint
        };

        return {
            ...session,
            fingerprint: nextFingerprint,
            sessionCookie: this.signSidPayload(sidPayload)
        };
    }

    public async deleteUser(session: MintedSession): Promise<void> {
        const keys = sessionRedisKeys(session.userId, session.sessionKeyHex);
        await this.withRedis(async (redis) => {
            await redis.del([keys.sentinelKey, keys.dataKey]);
        });
        this.createdSessions.delete(this.sessionIdentity(session.userId, session.sessionKeyHex));
    }

    public async teardownCreatedSessions(): Promise<void> {
        if (this.createdSessions.size === 0) {
            return;
        }
        const sessions = Array.from(this.createdSessions.values());
        await this.withRedis(async (redis) => {
            for (const session of sessions) {
                const [userId, sessionKeyHex] = session.split(':');
                const keys = sessionRedisKeys(userId, sessionKeyHex);
                await redis.del([keys.sentinelKey, keys.dataKey]);
            }
        });
        this.createdSessions.clear();
    }

    private signSidPayload(payload: object): string {
        const rawJson = JSON.stringify(payload);
        const digest = createHmac('sha256', this.signingKey).update(rawJson).digest('base64');
        return encodeURIComponent(`${digest}${rawJson}`);
    }

    private async withRedis<T>(fn: (redis: ReturnType<typeof createClient>) => Promise<T>): Promise<T> {
        const redis = createClient({ url: this.redisUrl });
        await redis.connect();
        try {
            return await fn(redis);
        } finally {
            await redis.disconnect();
        }
    }

    private sessionIdentity(userId: string, sessionKeyHex: string): string {
        return `${userId}:${sessionKeyHex}`;
    }
}

function decodeBase64UrlNoPad(value: string): Buffer {
    const normalized = value.replace(/-/g, '+').replace(/_/g, '/');
    const padding = normalized.length % 4 === 0 ? '' : '='.repeat(4 - (normalized.length % 4));
    return Buffer.from(`${normalized}${padding}`, 'base64');
}

function fingerprintFromUserAgent(userAgent: string): string {
    if (!userAgent) {
        return 'unknown';
    }
    const hash = createHash('sha256').update(userAgent).digest();
    return hash.toString('base64url');
}

function sessionRedisKeys(userId: string, sessionKeyHex: string): { sentinelKey: string; dataKey: string } {
    const userIdNoDash = userId.replace(/-/g, '');
    const keyHash = createHash('sha256').update(Buffer.from(sessionKeyHex, 'hex')).digest('hex');
    const prefix = `session:${userIdNoDash}:${keyHash}`;
    return {
        sentinelKey: `${prefix}:sentinel`,
        dataKey: `${prefix}:data`
    };
}
