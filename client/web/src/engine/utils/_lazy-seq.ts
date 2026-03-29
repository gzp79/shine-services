export type Mapper<T, U> = (value: T) => U;
export type Predicate<T> = (value: T) => boolean;
export type Reducer<T, U> = (acc: U, value: T) => U;

export class LazySeq<T> implements Iterable<T> {
    private readonly iteratorFactory: () => IterableIterator<T>;

    constructor(iteratorFactory: () => IterableIterator<T>) {
        this.iteratorFactory = iteratorFactory;
    }

    [Symbol.iterator](): IterableIterator<T> {
        return this.iteratorFactory();
    }

    map<U>(fn: Mapper<T, U>): LazySeq<U> {
        const parentFactory = this.iteratorFactory;

        return new LazySeq<U>(function* () {
            for (const x of parentFactory()) {
                yield fn(x);
            }
        });
    }

    filter(fn: Predicate<T>): LazySeq<T> {
        const parentFactory = this.iteratorFactory;

        return new LazySeq<T>(function* () {
            for (const x of parentFactory()) {
                if (fn(x)) yield x;
            }
        });
    }

    take(n: number): LazySeq<T> {
        const parentFactory = this.iteratorFactory;

        return new LazySeq<T>(function* () {
            if (n <= 0) return;
            let count = 0;
            for (const x of parentFactory()) {
                yield x;
                if (++count >= n) break;
            }
        });
    }

    skip(n: number): LazySeq<T> {
        const parentFactory = this.iteratorFactory;

        return new LazySeq<T>(function* () {
            let count = 0;
            const it = parentFactory();
            let next = it.next();
            while (!next.done && count++ < n) {
                next = it.next();
            }
            while (!next.done) {
                yield next.value;
                next = it.next();
            }
        });
    }

    reduce<U>(fn: Reducer<T, U>, initial: U): U {
        let acc = initial;
        for (const x of this) {
            acc = fn(acc, x);
        }
        return acc;
    }

    flatten<U>(this: LazySeq<Iterable<U>>): LazySeq<U> {
        const parent = this.iteratorFactory;

        return new LazySeq<U>(() => {
            const gen = (function* () {
                for (const inner of parent()) {
                    for (const value of inner) {
                        yield value;
                    }
                }
            })();

            return gen as IterableIterator<U>;
        });
    }

    toArray(): T[] {
        return Array.from(this);
    }
}

// Range constructor
export function range(from: number, to: number): LazySeq<number> {
    return new LazySeq<number>(function* () {
        for (let i = from; i < to; i++) yield i;
    });
}
