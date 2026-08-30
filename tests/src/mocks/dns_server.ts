import { debug } from 'debug';
import dgram from 'dgram';
import dns2 from 'dns2';
import os from 'os';

const { Packet } = dns2;

export interface DnsServerConfig {
    /** Zone answered authoritatively (apex + wildcard). Default: `local.scytta.com`. */
    zone?: string;
    /** IP returned for the zone. Default: the primary non-internal IPv4. */
    lanIp?: string;
    /** Upstream resolver for every other query. Default: `8.8.8.8`. */
    upstream?: string;
    /** TTL (seconds) for zone answers; short so a DHCP IP change recovers quickly. Default: 60. */
    ttl?: number;
    /** Listen port. Default: 53 (what a device's DNS setting expects; needs elevation). */
    port?: number;
    /** Upstream query timeout (ms). Default: 5000. */
    upstreamTimeoutMs?: number;
}

/** Picks the primary non-internal IPv4 address, matching os.networkInterfaces() order. */
export function detectLanIp(): string {
    for (const addresses of Object.values(os.networkInterfaces())) {
        for (const address of addresses ?? []) {
            if (address.family === 'IPv4' && !address.internal) {
                return address.address;
            }
        }
    }
    throw new Error('Could not auto-detect a LAN IPv4 address; set LAN_IP explicitly');
}

/**
 * A minimal LAN DNS resolver so physical devices (an iPhone with no hosts file) can reach the
 * dev PC by the `local.scytta.com` names. Answers the zone (apex + wildcard) with the PC's LAN IP
 * and forwards every other query to an upstream resolver so the device keeps working internet.
 */
export class DnsServer {
    public readonly name = 'dns';
    private readonly log: debug.Debugger;

    private readonly zone: string;
    private readonly lanIp: string;
    private readonly upstream: string;
    private readonly ttl: number;
    private readonly port: number;
    private readonly upstreamTimeoutMs: number;

    private server?: dns2.DnsServer;

    constructor(config: DnsServerConfig = {}) {
        this.log = debug(`test:mock:${this.name}`);
        this.zone = (config.zone ?? 'local.scytta.com').toLowerCase();
        this.lanIp = config.lanIp ?? detectLanIp();
        this.upstream = config.upstream ?? '8.8.8.8';
        this.ttl = config.ttl ?? 60;
        this.port = config.port ?? 53;
        this.upstreamTimeoutMs = config.upstreamTimeoutMs ?? 5000;
    }

    public get isRunning(): boolean {
        return this.server !== undefined;
    }

    public async start(): Promise<void> {
        if (this.isRunning) {
            throw new Error('Server has already been started');
        }

        this.log(`zone: *.${this.zone} (and apex) -> ${this.lanIp}, ttl ${this.ttl}s`);
        this.log(`upstream: ${this.upstream}`);

        const server = dns2.createServer({
            udp: true,
            tcp: true,
            handle: (request, send) => void this.handle(request, send)
        });

        // A bind failure surfaces via the 'error' event, not a rejected listen() — so race the two
        // and fail startup if the port is taken (e.g. by the Windows ICS service) or needs elevation.
        await new Promise<void>((resolve, reject) => {
            let settled = false;
            const settle = (fn: () => void) => {
                if (settled) return;
                settled = true;
                fn();
            };

            server.on('error', (error, transport) => {
                settle(() => {
                    void server.close().catch(() => {});
                    reject(new Error(`DNS ${transport} bind failed on port ${this.port}: ${error.message}`));
                });
                if (settled) this.log(`runtime error (${transport}): ${error.message}`);
            });

            server
                .listen({
                    udp: { port: this.port, address: this.lanIp },
                    tcp: { port: this.port, address: this.lanIp }
                })
                .then(() => settle(resolve))
                .catch((error: Error) => settle(() => reject(error)));
        });

        this.server = server;
        this.log(`Server started, listening on UDP+TCP ${this.lanIp}:${this.port}.`);
    }

    public async stop(): Promise<void> {
        if (!this.server) return;
        this.log('Stopping server ...');
        await this.server.close();
        this.server = undefined;
        this.log('Server stopped.');
    }

    private inZone(name: string): boolean {
        const lower = name.toLowerCase();
        return lower === this.zone || lower.endsWith(`.${this.zone}`);
    }

    private async handle(
        request: dns2.Packet,
        send: (response: dns2.Packet | Buffer) => Promise<Buffer>
    ): Promise<void> {
        const [question] = request.questions;
        if (!question) {
            void send(Packet.createResponseFromRequest(request));
            return;
        }

        const { name, type } = question;
        const typeName = Packet.TYPE_NAME[type] ?? String(type);

        if (this.inZone(name)) {
            const response = Packet.createResponseFromRequest(request);
            response.header.aa = 1;
            response.header.ra = 1;
            if (type === Packet.TYPE.A) {
                response.answers.push(
                    Packet.createResourceFromQuestion(question, {
                        type: Packet.TYPE.A,
                        class: Packet.CLASS.IN,
                        ttl: this.ttl,
                        address: this.lanIp
                    })
                );
                this.log(`${name} ${typeName} -> ${this.lanIp}`);
            } else {
                // Empty NOERROR (notably for AAAA) so the client falls back to A. Never forward the
                // zone upstream, where it would resolve to the wrong address.
                this.log(`${name} ${typeName} -> NOERROR (empty)`);
            }
            void send(response);
            return;
        }

        try {
            const answer = await this.forward(request);
            this.log(`${name} ${typeName} -> upstream (${this.upstream})`);
            void send(answer);
        } catch (error) {
            const response = Packet.createResponseFromRequest(request);
            response.header.rcode = Packet.RCODE.SERVFAIL;
            this.log(`${name} ${typeName} -> upstream failed: ${(error as Error).message}`);
            void send(response);
        }
    }

    /** Relays the raw query bytes to upstream and returns the raw response, preserving every
     *  record type (HTTPS/SVCB, EDNS, ...) exactly as the upstream produced them. */
    private forward(request: dns2.Packet): Promise<Buffer> {
        return new Promise<Buffer>((resolve, reject) => {
            const socket = dgram.createSocket('udp4');
            const cleanup = () => {
                clearTimeout(timer);
                socket.close();
            };
            const timer = setTimeout(() => {
                cleanup();
                reject(new Error('upstream timeout'));
            }, this.upstreamTimeoutMs);

            socket.once('message', (message) => {
                cleanup();
                resolve(message);
            });
            socket.once('error', (error) => {
                cleanup();
                reject(error);
            });
            socket.send(request.toBuffer(), 53, this.upstream, (error) => {
                if (error) {
                    cleanup();
                    reject(error);
                }
            });
        });
    }
}
