import { ApiClient } from '$lib/api/api';
import { SessionMint } from '$lib/mocks/session_mint';
import { joinURL, waitForCondition } from '$lib/utils';
import { Toxiproxy } from 'toxiproxy-node-client';

type Target = 'postgres' | 'redis';

type ProxyState = { enabled: boolean; toxics: unknown[] };

export type OutageContext = {
    toxiproxyUrl: string;
    /** Base urls of the services that may be running. Those not running are left out of the checks. */
    serviceUrls: string[];
    client: ApiClient;
};

/** The test timeout leaves no room for more. */
const RECOVERY_TIMEOUT_MS = 15_000;

/** The connection is refused or the host is unknown, so nothing is listening there. */
const NOT_LISTENING = /ECONNREFUSED|ENOTFOUND|EHOSTUNREACH/;

type PoolStatus = {
    connections: number;
    healthy: boolean;
    listener: { connected: boolean; channels: number };
};

function isPoolStatus(value: unknown): value is PoolStatus {
    const pool = value as Partial<PoolStatus> | null;
    return (
        typeof pool?.connections === 'number' &&
        typeof pool.healthy === 'boolean' &&
        typeof pool.listener?.connected === 'boolean'
    );
}

type ServiceCheck = {
    /** What stops the service from working with its database and cache. */
    problems: string[];
    /** The largest pool of the service. */
    connections: number;
};

/**
 * Asks a service how it works with its database and cache. A service that is not running has no problems, as
 * that is not caused by the infra.
 *
 * A pool is a problem when the round trip on one of its connections fails; the pool drops that connection, so
 * asking again tries the next one. A listener is a problem when it has channels but no connection.
 */
async function checkService(url: string, client: ApiClient, sessionCookie: string): Promise<ServiceCheck> {
    const fine: ServiceCheck = { problems: [], connections: 0 };

    try {
        // Answers without touching the database. Anything but a 200 is a proxy in front of a stopped service.
        const ready = await client.get(joinURL(url, '/info/ready'));
        if (ready.status() !== 200) {
            return fine;
        }
    } catch (error) {
        const reason = error instanceof Error ? error.message : String(error);
        return NOT_LISTENING.test(reason) ? fine : { ...fine, problems: [`${url} is not reachable: ${reason}`] };
    }

    // The request itself reads the session through the service's session pool.
    const response = await client.get(joinURL(url, '/info/status')).withCookies({ sid: sessionCookie });
    if (response.status() !== 200) {
        return { ...fine, problems: [`${url}: status answered ${response.status()}`] };
    }

    const check: ServiceCheck = { problems: [], connections: 0 };
    for (const [name, value] of Object.entries((await response.json()) as Record<string, unknown>)) {
        if (!isPoolStatus(value)) {
            continue;
        }
        check.connections = Math.max(check.connections, value.connections);
        if (!value.healthy) {
            check.problems.push(`${url}: the ${name} pool is not healthy`);
        }
        if (value.listener.channels > 0 && !value.listener.connected) {
            check.problems.push(`${url}: the ${name} listener is not connected`);
        }
    }
    return check;
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
 * The proxies are shared, so every running service is hit, not only the one the test talks to. Once the test is
 * over the outage is cleared, then each running service is asked until all of its pools and listeners work
 * again. Cutting the proxied connections leaves dead sockets in the pools, evicted only once something
 * stumbles on them, so this is what stops the outage from spilling into the next test.
 *
 * Only one outage can be active at a time, as two of them would break the test for sure.
 */
export class InfraOutage {
    private readonly toxiproxy: Toxiproxy;
    private readonly active = new Map<Target, string>();
    private injected = false;

    constructor(private readonly context: OutageContext) {
        this.toxiproxy = new Toxiproxy(context.toxiproxyUrl);
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

        try {
            await this.waitForServices();
        } catch (error) {
            const reason = error instanceof Error ? error.message : String(error);
            throw new Error(`The services did not recover from the infra outage: ${reason}`);
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

    /**
     * Waits until every running service is free of problems. A check goes through a single pooled connection, so
     * the services have to be clean for as many checks in a row as their pool has connections, to be sure none
     * of the connections cut by the outage is left.
     */
    private async waitForServices(): Promise<void> {
        const { serviceUrls, client } = this.context;
        const mint = await SessionMint.fromServerConfig();
        try {
            const admin = await mint.createUserSession({ roles: ['SuperAdmin'] });
            let clean = 0;
            await waitForCondition(
                async () => {
                    const checks = await Promise.all(
                        serviceUrls.map((url) => checkService(url, client, admin.sessionCookie))
                    );
                    const problems = checks.flatMap((check) => check.problems);
                    if (problems.length > 0) {
                        clean = 0;
                        throw new Error(problems.join('; '));
                    }
                    clean += 1;
                    const needed = Math.max(...checks.map((check) => check.connections), 0) + 1;
                    return clean >= needed ? true : undefined;
                },
                { timeout: RECOVERY_TIMEOUT_MS, interval: 200 }
            );
        } finally {
            await mint.teardownCreatedSessions();
        }
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
            try {
                await this.waitForServices();
            } catch (error) {
                const reason = error instanceof Error ? error.message : String(error);
                throw new Error(`The services are not working, an outage cannot start: ${reason}`);
            }
        }
        // Set before the proxy is touched: whatever a half done injection leaves behind has to be cleared.
        this.injected = true;
        this.active.set(target, mode);
    }
}
