export function delay(ms: number): Promise<void> {
    return new Promise((resolve) => setTimeout(resolve, ms));
}

export const DEFAULT_USER_AGENT = 'shine-api-test-client';
