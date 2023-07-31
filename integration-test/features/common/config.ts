export const baseServiceUrl =
    process.env.SHINE_BASE_API_URL ?? 'https://cloud.scytta.com';

export const baseUrls = {
    base: baseServiceUrl,
    service: baseServiceUrl + '/identity',
    doc: baseServiceUrl + '/identity/doc',
    api: baseServiceUrl + '/identity/api'
};
