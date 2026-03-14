import crypto from 'crypto';

export function getSHA256Hash(text: string): string {
    const hash = crypto.createHash('sha256');
    hash.update(text);
    return hash.digest('hex');
}
