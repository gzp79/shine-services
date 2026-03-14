export function convertKeysToLowerCase<T>(obj: Record<string, T>): Record<string, T> {
    return Object.keys(obj).reduce(
        (acc, key) => {
            acc[key.toLowerCase()] = obj[key];
            return acc;
        },
        {} as Record<string, T>
    );
}

export function removeUndefinedValues<T>(obj: Record<string, T | undefined>): Record<string, T> {
    return Object.keys(obj).reduce(
        (acc, key) => {
            if (obj[key] !== undefined) {
                acc[key] = obj[key];
            }
            return acc;
        },
        {} as Record<string, T>
    );
}
