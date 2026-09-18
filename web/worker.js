// Web Worker hosting the wasm optimizer and the HiGHS solver. Solving can take long enough that
// running it on the main thread would freeze the page's own rendering, which is what makes the
// browser offer to kill the tab; every wasm call is dispatched through here instead, so the main
// thread stays free to paint a progress indicator while a solve is in flight. See `web/app.js`'s
// `callWorker` for the request/response contract this expects.
import highsModule from './vendor/highs/highs.mjs';

// The page starts this worker as `worker.js?load=<page load time>` (see app.js), and the wasm
// module is loaded with the same query and a revalidated fetch, so a page never pairs new page
// code with an older cached solver.
// The message handler below is installed right away and waits on this, so no request can arrive
// before there's a handler for it.
const load = new URL(import.meta.url).search;
const ready = import('./pkg/aniimax.js' + load).then(async (pkg) => {
    await pkg.default({ module_or_path: fetch(new URL('./pkg/aniimax_bg.wasm', import.meta.url), { cache: 'no-cache' }) });
    return pkg;
});

// Handlers taking a single string argument and returning one; `find_plan` is handled separately
// below since it also takes a progress callback.
const HANDLER_NAMES = ['time_to_reach', 'get_version', 'get_all_items'];

// HiGHS (https://highs.dev), compiled to WebAssembly. A fresh instance per solve, from bytes
// fetched once, so one failed solve can't leave a broken instance behind for the next.
let highsBytes = null;
async function newHighs() {
    if (!highsBytes) {
        highsBytes = fetch(new URL('./vendor/highs/highs.wasm', import.meta.url)).then(r => {
            if (!r.ok) throw new Error(`Could not load HiGHS (${r.status})`);
            return r.arrayBuffer();
        });
    }
    return highsModule({ wasmBinary: await highsBytes });
}

// Seconds HiGHS may search before settling for the best plan found so far.
const EXACT_TIME_LIMIT = 30;

// Solves one exact-planner model with HiGHS: `{ values, proven, objective }`, or null if HiGHS
// found no plan at all.
async function solveModel(problem) {
    const highs = await newHighs();
    const result = highs.solve(problem.lp, { mip_rel_gap: 0, time_limit: EXACT_TIME_LIMIT });
    const proven = result.Status === 'Optimal';
    if (!proven && result.Status !== 'Time limit reached') return null;
    const values = Array.from({ length: problem.variables }, (_, i) => result.Columns['x' + i]?.Primal ?? 0);
    return { values, proven, objective: result.ObjectiveValue };
}

// The exact planner (see `exact_problem` in wasm.rs): builds the model in wasm, solves it with
// HiGHS, and turns the answer back into a plan. With "prioritize byproducts" on, it first finds
// the most of each byproduct the facilities can make and requires the plan to keep that much.
// For the level-up strategy, it first finds the soonest level-up and requires the plan to keep
// that pace; if the facilities can't make the level-up at all, the plan is for coins and says so.
// Returns the plan's JSON, or throws with the reason it couldn't, so the caller can fall back to
// `find_plan` and say why.
async function exactPlanJson(pkg, payload) {
    const { exact_byproduct_problems, exact_level_up_problem, exact_problem, exact_plan } = pkg;
    const stage = { floors: [] };
    let allProven = true;
    for (const problem of JSON.parse(exact_byproduct_problems(payload))) {
        const most = await solveModel(problem);
        if (!most) throw new Error(`no plan found for the most ${problem.resource}`);
        allProven &&= most.proven;
        stage.floors.push([problem.resource, most.objective]);
    }
    let levelUpNote = null;
    const levelUp = JSON.parse(exact_level_up_problem(payload));
    if (levelUp.lp) {
        const fastest = await solveModel(levelUp);
        if (!fastest) throw new Error('no plan found for the level-up');
        if (fastest.objective > 1e-9) {
            allProven &&= fastest.proven;
            stage.pace = fastest.objective;
        } else {
            levelUpNote = 'unreachable';
        }
    }
    let stageJson = JSON.stringify(stage);
    let problem = JSON.parse(exact_problem(payload, stageJson));
    if (!problem.lp) throw new Error('this setup isn\'t covered by the exact planner');
    let solved = await solveModel(problem);
    if (!solved) throw new Error('the solver found no plan');
    let proven = solved.proven && allProven;
    let bound = solved.objective;
    if (!proven) {
        // The same model without whole units: the most any plan could earn.
        const relaxed = (await newHighs()).solve(problem.lp.replace(/\nGeneral\n[\s\S]*\nEnd/, '\nEnd'), {});
        bound = relaxed.ObjectiveValue;
    }
    if (stage.pace) {
        // Keeping that pace and those coins, spare Bench and Kiln time goes to the level-up.
        stage.coins = solved.objective;
        stageJson = JSON.stringify(stage);
        problem = JSON.parse(exact_problem(payload, stageJson));
        const stocked = await solveModel(problem);
        if (!stocked) throw new Error('no plan found for the level-up stock');
        solved = stocked;
        proven &&= stocked.proven;
    }
    const json = exact_plan(payload, stageJson, JSON.stringify({ values: solved.values, proven, bound }));
    const plan = JSON.parse(json);
    if (!plan.success) throw new Error(plan.error || 'the plan failed its check');
    if (!levelUpNote) return json;
    plan.level_up_note = levelUpNote;
    return JSON.stringify(plan);
}

self.onmessage = async (event) => {
    const { id, type, payload } = event.data;
    try {
        const pkg = await ready;
        if (type === 'find_plan') {
            let result = null;
            let fallbackReason = null;
            try {
                result = await exactPlanJson(pkg, payload);
            } catch (error) {
                fallbackReason = error && error.message ? error.message : String(error);
                console.warn('Exact planner failed; using the backup planner instead:', error);
            }
            if (!result) {
                // Forwarded straight from the wasm solver's own real trial-solve count (see
                // `find_plan`'s doc comment in wasm.rs); a `type: 'progress'` message, distinct
                // from the final `{ ok, result }` response below, so `app.js`'s `callWorker` can
                // relay it to a live progress bar without resolving the request early.
                const onProgress = (count) => self.postMessage({ id, type: 'progress', count });
                const plan = JSON.parse(pkg.find_plan(payload, onProgress));
                plan.fallback_reason = fallbackReason;
                result = JSON.stringify(plan);
            }
            self.postMessage({ id, ok: true, result });
            return;
        }
        const handler = HANDLER_NAMES.includes(type) ? pkg[type] : null;
        if (!handler) {
            throw new Error(`Unknown worker request type: ${type}`);
        }
        const result = payload === undefined ? handler() : handler(payload);
        self.postMessage({ id, ok: true, result });
    } catch (error) {
        self.postMessage({ id, ok: false, error: error && error.message ? error.message : String(error) });
    }
};
