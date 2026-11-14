import type { FullConfig } from '@playwright/test';
import { resetToxiproxy } from './infra_outage';

export default async function globalSetup(config: FullConfig): Promise<void> {
    for (const project of config.projects) {
        const { toxiproxyUrl } = project.use as { toxiproxyUrl?: string };
        if (!toxiproxyUrl) {
            continue;
        }
        try {
            // A crashed run can leave an outage behind, and an outage already there fails the tests that start a new one.
            await resetToxiproxy(toxiproxyUrl);
        } catch {
            // Ignore errors during reset, as the proxy might not be running.
        }
    }
}
