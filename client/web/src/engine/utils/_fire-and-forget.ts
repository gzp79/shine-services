// Marks a promise as intentionally unhandled by the caller (e.g. a sync DOM handler triggering
// async work). The callee is expected to report its own errors; a rejection here goes unnoticed.
export function fireAndForget(promise: Promise<unknown>): void {
    void promise;
}
