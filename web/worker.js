// Web Worker hosting the wasm optimizer. `find_plan` can take long enough on a complex facility
// setup that running it on the main thread freezes the page's own rendering, which is what makes
// the browser offer to kill the tab; every wasm call is dispatched through here instead, so the
// main thread stays free to paint a progress indicator while a solve is in flight. See
// `web/app.js`'s `callWorker` for the request/response contract this expects.
import init, { find_plan, time_to_reach, get_version, get_all_items } from './pkg/aniimax.js';

const ready = init();

// Handlers taking a single string argument and returning one; `find_plan` is handled separately
// below since it also takes a progress callback.
const HANDLERS = { time_to_reach, get_version, get_all_items };

self.onmessage = async (event) => {
    const { id, type, payload } = event.data;
    try {
        await ready;
        if (type === 'find_plan') {
            // Forwarded straight from the wasm solver's own real trial-solve count (see
            // `find_plan`'s doc comment in wasm.rs); a `type: 'progress'` message, distinct from
            // the final `{ ok, result }` response below, so `app.js`'s `callWorker` can relay it
            // to a live progress bar without resolving the request early.
            const onProgress = (count) => self.postMessage({ id, type: 'progress', count });
            const result = find_plan(payload, onProgress);
            self.postMessage({ id, ok: true, result });
            return;
        }
        const handler = HANDLERS[type];
        if (!handler) {
            throw new Error(`Unknown worker request type: ${type}`);
        }
        const result = payload === undefined ? handler() : handler(payload);
        self.postMessage({ id, ok: true, result });
    } catch (error) {
        self.postMessage({ id, ok: false, error: error && error.message ? error.message : String(error) });
    }
};
