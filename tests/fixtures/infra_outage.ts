import { AuthAPI, DefaultRedirects } from '$lib/api/auth_api';
import { StreamSession, wsConnectUrl, wsHeaders } from '$lib/api/builder_ws';
import { SessionMint } from '$lib/mocks/session_mint';
import { waitForCondition } from '$lib/utils';
import { randomUUID } from 'node:crypto';
import { Toxiproxy } from 'toxiproxy-node-client';

/** The services a test relies on to work again after an outage. */
export type OutageService = 'identity' | 'builder';

type Target = 'postgres' | 'redis';

type ProxyState = { enabled: boolean; toxics: unknown[] };

export type OutageContext = {
    toxiproxyUrl: string;
    homeUrl: string;
    identityUrl: string;
    builderWSUrl: string;
    defaultRedirects: DefaultRedirects;
    enableRequestLogging: boolean;
};

/** A request that reaches the database and the cache of a service. */
type ServiceProbe = { service: OutageService; check: () => Promise<void> };

function serviceProbe(service: OutageService, context: OutageContext): ServiceProbe {
    switch (service) {
        case 'identity':
            return {
                service,
                check: async () => {
                    const auth = new AuthAPI(
                        context.identityUrl,
                        context.defaultRedirects,
                        context.enableRequestLogging
                    );
                    await auth.loginAsGuestUser();
                }
            };
        case 'builder':
            return {
                service,
                // A stream subscription reaches the session cache, the stream cache and the database.
                check: async () => {
                    const mint = await SessionMint.fromServerConfig();
                    try {
                        const user = await mint.createUserSession({ userId: randomUUID() });
                        const session = await StreamSession.open(wsConnectUrl(context.builderWSUrl), {
                            sid: user.sessionCookie,
                            ...wsHeaders(context.homeUrl, context.builderWSUrl)
                        });
                        try {
                            await session.subscribe(`chat:probe-${randomUUID().replace(/-/g, '')}`);
                        } finally {
                            session.close();
                        }
                    } finally {
                        await mint.teardownCreatedSessions();
                    }
                }
            };
    }
}

async function injectedFaults(toxiproxyUrl: string): Promise<string[]> {
    const response = await fetch(`${toxiproxyUrl}/proxies`);
    const proxies = (await response.json()) as Record<string, ProxyState>;
    return Object.entries(proxies)
        .filter(([, proxy]) => !proxy.enabled || proxy.toxics.length > 0)
        .map(([name]) => name);
}

/** Puts toxiproxy into a clean state; whatever a crashed run left behind is gone. */
export async function resetToxiproxy(toxiproxyUrl: string): Promise<void> {
    await new Toxiproxy(toxiproxyUrl).reset();
}

/**
 * Breaks the database or the cache the services reach through toxiproxy, for one test.
 *
 * The services named at creation have to work again once the test is over: the outage is cleared, then each
 * of them is asked for something that needs its database and its cache until it answers. Cutting the proxied
 * connections leaves dead sockets in the pools of the services, evicted only once a request stumbles on them,
 * so this is what stops the outage from spilling into the next test.
 *
 * Only one outage can be active at a time, as two of them would break the test for sure.
 */
export class InfraOutage {
    private readonly toxiproxy: Toxiproxy;
    private readonly probes: ServiceProbe[];
    private readonly active = new Map<Target, string>();
    private injected = false;

    constructor(
        private readonly context: OutageContext,
        services: OutageService[]
    ) {
        this.toxiproxy = new Toxiproxy(context.toxiproxyUrl);
        this.probes = services.map((service) => serviceProbe(service, context));
    }

    async stopPostgres(): Promise<void> {
        await this.stop('postgres');
    }

    async startPostgres(): Promise<void> {
        await this.start('postgres');
    }

    /** Every answer of the database takes the given time. It lasts until the end of the test. */
    async slowPostgres(latencyMs: number): Promise<void> {
        await this.slow('postgres', latencyMs);
    }

    async stopRedis(): Promise<void> {
        await this.stop('redis');
    }

    async startRedis(): Promise<void> {
        await this.start('redis');
    }

    /** Every answer of the cache takes the given time. It lasts until the end of the test. */
    async slowRedis(latencyMs: number): Promise<void> {
        await this.slow('redis', latencyMs);
    }

    /** Clears the outage and waits for the services to work again. Called by the fixture. */
    async dispose(): Promise<void> {
        if (!this.injected) {
            return;
        }
        await this.toxiproxy.reset();
        this.active.clear();

        for (const { service, check } of this.probes) {
            // waitForCondition retries until the answer is not undefined.
            let lastError: unknown;
            try {
                await waitForCondition(
                    async () => {
                        try {
                            await check();
                            return true;
                        } catch (error) {
                            lastError = error;
                            throw error;
                        }
                    },
                    { timeout: 15000 }
                );
            } catch {
                const reason = lastError instanceof Error ? lastError.message : String(lastError);
                throw new Error(`${service} did not recover from the infra outage: ${reason}`);
            }
        }
    }

    private async stop(target: Target): Promise<void> {
        await this.begin(target, 'stopped');
        const proxy = await this.toxiproxy.get(target);
        await proxy.update({ enabled: false, listen: proxy.listen, upstream: proxy.upstream });
    }

    private async start(target: Target): Promise<void> {
        if (this.active.get(target) !== 'stopped') {
            throw new Error(`Cannot start ${target}: it was not stopped by this test`);
        }
        const proxy = await this.toxiproxy.get(target);
        await proxy.update({ enabled: true, listen: proxy.listen, upstream: proxy.upstream });
        this.active.delete(target);
    }

    private async slow(target: Target, latencyMs: number): Promise<void> {
        await this.begin(target, 'slow');
        const proxy = await this.toxiproxy.get(target);
        await proxy.addToxic({
            name: 'slow',
            type: 'latency',
            stream: 'downstream',
            toxicity: 1.0,
            attributes: { latency: latencyMs, jitter: 0 }
        });
    }

    /** Checks that nothing else is broken and the services are fine, then marks the target as broken. */
    private async begin(target: Target, mode: string): Promise<void> {
        if (this.active.size > 0) {
            const current = Array.from(this.active.entries(), ([name, state]) => `${name} is ${state}`).join(', ');
            throw new Error(`Cannot make ${target} ${mode}: ${current}. Two outages at once break the test.`);
        }
        if (!this.injected) {
            const leftover = await injectedFaults(this.context.toxiproxyUrl);
            if (leftover.length > 0) {
                throw new Error(
                    `Cannot make ${target} ${mode}: toxiproxy already has a fault on ${leftover.join(', ')}, ` +
                        'left by another test or run'
                );
            }
            for (const { service, check } of this.probes) {
                try {
                    await check();
                } catch (error) {
                    const reason = error instanceof Error ? error.message : String(error);
                    throw new Error(`The ${service} service is not working, an outage cannot start: ${reason}`);
                }
            }
        }
        // Set before the proxy is touched: whatever a half done injection leaves behind has to be cleared.
        this.injected = true;
        this.active.set(target, mode);
    }
}
