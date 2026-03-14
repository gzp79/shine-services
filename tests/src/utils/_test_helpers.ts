/**
 * Test utility functions for reliable async operations
 */

/**
 * Wait for a condition with polling
 * Replaces all hard-coded sleeps
 */
export async function waitForCondition<T>(
    fn: () => Promise<T>,
    options: {
        timeout?: number; // default: 5000ms
        interval?: number; // default: 100ms
        errorMessage?: string;
    } = {}
): Promise<T> {
    const timeout = options.timeout ?? 5000;
    const interval = options.interval ?? 100;
    const start = Date.now();

    while (true) {
        try {
            const result = await fn();
            if (result !== undefined) return result;
        } catch (e) {
            const errorDetails = e instanceof Error ? e.message : String(e);
            if (Date.now() - start > timeout) {
                throw new Error(
                    options.errorMessage ?? `Condition not met within ${timeout}ms. Last error: ${errorDetails}`
                );
            }
        }

        if (Date.now() - start > timeout) {
            throw new Error(options.errorMessage ?? `Condition not met within ${timeout}ms`);
        }

        await new Promise((resolve) => setTimeout(resolve, interval));
    }
}

/**
 * Verify service recovery after failure injection
 */
export async function verifyRecovery<T = void>(
    operation: () => Promise<T>,
    options: { maxAttempts?: number; delay?: number } = {}
): Promise<T | void> {
    const maxAttempts = options.maxAttempts ?? 3;

    for (let i = 0; i < maxAttempts; i++) {
        try {
            const result = await operation();
            return result; // Success
        } catch (e) {
            if (i === maxAttempts - 1) {
                const errorDetails = e instanceof Error ? e.message : String(e);
                throw new Error(`Operation failed after ${maxAttempts} attempts. Last error: ${errorDetails}`);
            }
            await new Promise((resolve) => setTimeout(resolve, options.delay ?? 1000));
        }
    }
}
