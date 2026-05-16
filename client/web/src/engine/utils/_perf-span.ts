const enabled = true;

class ActiveSpan implements Disposable {
    private readonly startMark: string;

    constructor(private readonly name: string) {
        this.startMark = `${name}:s`;
        performance.mark(this.startMark);
    }

    [Symbol.dispose](): void {
        const endMark = `${this.name}:e`;
        performance.mark(endMark);
        performance.measure(this.name, this.startMark, endMark);
        performance.clearMarks(this.startMark);
        performance.clearMarks(endMark);
    }
}

const NOOP: Disposable = { [Symbol.dispose]() {} };

export function span(name: string): Disposable {
    return enabled ? new ActiveSpan(name) : NOOP;
}
