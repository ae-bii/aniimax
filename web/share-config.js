// A versioned, client-only snapshot. Fragments are not sent with HTTP requests.
const SHARE_VERSION = 'v1';
const MAX_TOKEN_LENGTH = 100_000;
const MAX_CONFIG_BYTES = 1_000_000;

async function readBounded(stream, maxBytes) {
    const reader = stream.getReader();
    const chunks = [];
    let total = 0;
    while (true) {
        const { done, value } = await reader.read();
        if (done) break;
        total += value.length;
        if (total > maxBytes) {
            await reader.cancel();
            throw new Error('Shared config is too large');
        }
        chunks.push(value);
    }
    const bytes = new Uint8Array(total);
    let offset = 0;
    for (const chunk of chunks) {
        bytes.set(chunk, offset);
        offset += chunk.length;
    }
    return bytes;
}

export async function createShareUrl(href, config) {
    const json = new TextEncoder().encode(JSON.stringify(config));
    if (json.length > MAX_CONFIG_BYTES) throw new Error('Shared config is too large');
    const compressed = await readBounded(
        new Blob([json]).stream().pipeThrough(new CompressionStream('gzip')),
        MAX_TOKEN_LENGTH
    );
    const encoded = btoa(Array.from(compressed, byte => String.fromCharCode(byte)).join(''))
        .replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
    const token = `${SHARE_VERSION}.${encoded}`;
    if (token.length > MAX_TOKEN_LENGTH) throw new Error('Shared config is too large for a link');
    const url = new URL(href);
    url.hash = new URLSearchParams({ config: token }).toString();
    return url.href;
}

// Returns null when this URL has no shared config; rejects a damaged or unknown version.
export async function readShareHash(hash) {
    const params = new URLSearchParams(hash.replace(/^#/, ''));
    if (!params.has('config')) return null;
    const token = params.get('config');
    if (!token || token.length > MAX_TOKEN_LENGTH || !/^v1\.[A-Za-z0-9_-]+$/.test(token)) {
        throw new Error('Invalid shared config');
    }
    const encoded = token.slice(SHARE_VERSION.length + 1);
    const binary = atob(encoded.replace(/-/g, '+').replace(/_/g, '/'));
    const bytes = Uint8Array.from(binary, char => char.charCodeAt(0));
    const decompressed = await readBounded(
        new Blob([bytes]).stream().pipeThrough(new DecompressionStream('gzip')),
        MAX_CONFIG_BYTES
    );
    const config = JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(decompressed));
    if (!config || typeof config !== 'object' || Array.isArray(config)
        || !config.facilityTiers || typeof config.facilityTiers !== 'object' || Array.isArray(config.facilityTiers)) {
        throw new Error('Invalid shared config');
    }
    return config;
}

// Keep any unrelated fragment parameters when a shared setup is edited or cleared.
export function urlWithoutShare(href) {
    const url = new URL(href);
    const params = new URLSearchParams(url.hash.replace(/^#/, ''));
    if (!params.has('config')) return url.href;
    params.delete('config');
    url.hash = params.toString();
    return url.href;
}
