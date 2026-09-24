// Aniimax Web Application

import {
    FACILITIES, FACILITY_CATEGORIES, FACILITY_CATEGORY_BY_NAME,
    MAX_HOME_LEVEL, ANIIMO_MAX, simpleSetup,
    LEVEL_UP_COSTS, LEVEL_UP_CHAINS, SPECIAL_RECIPES, ANIIPOD_TIERS,
} from './facility-config.js';

let wasmReady = false;

// The wasm optimizer runs in a Web Worker (see worker.js), not on the main thread: `find_plan`
// can take long enough on a complex facility setup that running it here would freeze the page's
// own rendering, which is what makes the browser offer to kill the tab. Every call site below
// goes through `callWorker` instead of calling a wasm-bindgen function directly.
let worker = null;
let nextRequestId = 0;
const pendingWorkerRequests = new Map();

// Tags this page load's worker (and, through it, the wasm solver; see worker.js) so the browser
// never runs a cached older solver next to newer page code.
const WORKER_URL = `./worker.js?load=${Date.now()}`;

function initWorker() {
    worker = new Worker(WORKER_URL, { type: 'module' });
    worker.onmessage = (event) => {
        const { id, type, ok, result, error, count } = event.data;
        const pending = pendingWorkerRequests.get(id);
        if (!pending) return;
        // A `find_plan` request can receive several `type: 'progress'` messages (the solver's own
        // real trial-solve count; see `find_plan`'s doc comment in wasm.rs) before its one final
        // `{ ok, result }` response; only the latter resolves/removes the pending request.
        if (type === 'progress') {
            if (pending.onProgress) pending.onProgress(count);
            return;
        }
        pendingWorkerRequests.delete(id);
        if (ok) {
            pending.resolve(result);
        } else {
            pending.reject(new Error(error));
        }
    };
    worker.onerror = (event) => {
        console.error('Worker error:', event.message || event);
    };
}

// Throws away the worker and anything still running in it, e.g. a Minimum-setup solve from an
// older Calculate click that would otherwise hold up the new one, and starts a fresh worker.
function restartWorker() {
    worker.terminate();
    pendingWorkerRequests.forEach(pending => pending.reject(new Error('Cancelled by a newer calculation')));
    pendingWorkerRequests.clear();
    initWorker();
}

// Sends one request to the worker and resolves with its result (or rejects with its error);
// `type` matches a key in worker.js's `HANDLERS` (or `'find_plan'`, handled specially there),
// `payload` is that function's own single string argument (omit for `get_version`/
// `get_all_items`, which take none). `onProgress(count)`, if given, is called for every
// intermediate progress message the request receives before its final result (currently only
// `find_plan` sends any); see worker.js.
function callWorker(type, payload, onProgress) {
    return new Promise((resolve, reject) => {
        const id = ++nextRequestId;
        pendingWorkerRequests.set(id, { resolve, reject, onProgress });
        worker.postMessage({ id, type, payload });
    });
}

// Converts the solver's real, running trial-solve count (from `find_plan`'s progress callback;
// see worker.js) into a progress-bar fill percentage. The algorithm's exact total trial count
// isn't knowable in advance: several of its exclusion passes stop once they converge rather than
// running a fixed number of times (see `find_production_plan`'s doc comments in optimizer.rs), so
// there's no true denominator to divide by. Every tick this responds to is a genuinely completed
// trial solve, so unlike a purely decorative animation, a faster machine or a simpler facility
// setup visibly reaches each milestone sooner. Capped at 96% (not 100%) while still running, so
// the bar never visually claims "done" before `find_plan` actually returns; `runFindPlan` sets it
// to a literal 100% only once the result is actually back.
//
// Two phases, not one asymptotic curve for the whole run: the solver's own shape is bimodal, not
// smoothly decaying. The first `EARLY_PHASE_TRIALS` or so cover just the initial candidate
// solve (fast, and roughly the ENTIRE cost for a simple facility setup with nothing contested).
// Everything past that is the environment-coverage-CHOICE exclusion pass (see its doc comment in
// optimizer.rs), which reruns the full packing pipeline per trial and, for a facility setup with
// real contested resources, routinely runs into the hundreds of trials across its rounds of
// per-processor, per-ingredient, and pairs searches. A single asymptotic curve tuned to feel right
// for the fast, simple case (a low halfway trial count) makes that expensive long tail nearly
// invisible -- it's already past 90% by trial 100, then creeps for the remaining several hundred,
// which is exactly the "gets exponentially slower towards the end" complaint this two-phase
// version fixes: the SECOND phase gets its own, much larger halfway trial count, so the visual
// progress keeps moving noticeably through that long tail instead of flatlining near the cap.
const EARLY_PHASE_TRIALS = 15;
const EARLY_PHASE_PERCENT = 25;
const LATE_PHASE_HALFWAY_TRIALS = 120;
function trialCountToPercent(count) {
    if (count <= EARLY_PHASE_TRIALS) {
        return Math.round((EARLY_PHASE_PERCENT * count) / EARLY_PHASE_TRIALS);
    }
    const trialsIntoLatePhase = count - EARLY_PHASE_TRIALS;
    const latePhasePercentRange = 96 - EARLY_PHASE_PERCENT;
    const latePhaseFraction = trialsIntoLatePhase / (trialsIntoLatePhase + LATE_PHASE_HALFWAY_TRIALS);
    return Math.min(96, Math.round(EARLY_PHASE_PERCENT + latePhasePercentRange * latePhaseFraction));
}

// The most recently computed plan (the full JS object returned by find_plan, including
// `success`/`error`); held in memory so changing the goal amount can call time_to_reach directly
// without re-running the facility-allocation solve. Cleared whenever facilities/currency/modules
// change, since those invalidate the plan.
let lastPlan = null;

// Plans for both Aniimo setups from the latest Calculate: `{ best, minimum }`. Best is solved and
// shown first; Minimum follows in the background (see `runFindPlan`). `planRunId` lets a newer
// Calculate click discard an older run's late Minimum result.
let plansBySetup = {};
let planRunId = 0;

function selectedAniimoSetup() {
    return document.getElementById('aniimo-minimum').checked ? 'minimum' : 'best';
}

// Shows the plan for the selected Aniimo setup, or a "still working" note if Minimum isn't ready.
function showSelectedPlan(scroll) {
    const setup = selectedAniimoSetup();
    const plan = plansBySetup[setup];
    const pending = document.getElementById('aniimo-pending');
    if (!plan) {
        pending.style.display = 'block';
        return;
    }
    pending.style.display = 'none';
    lastPlan = plan;
    displayPlan(plan, scroll);
    if (plan.success) runTimeToGoal();
}

// The most recently computed goal result, held the same way as `lastPlan` so switching the rate
// unit can re-render the Product Breakdown table's Profit column without recomputing the goal.
let lastGoalResult = null;

// Display name for each optimizable currency. Coins are the only one since the full release
// removed Bud Tickets; kept as a map so a plan's `currency` still resolves to its label.
const CURRENCY_LABELS = {
    coins: 'Coins',
    aniimo_exp: 'Aniimo EXP',
    aniipods: 'Aniipods',
};

// Multiplier from the solver's native per-second rate to each display unit, and the short suffix
// shown next to the currency label (e.g. "Coins/hour"). "Your Rate" is stored and computed
// per-second throughout; this only affects how that one number is displayed.
const RATE_UNIT_SECONDS = {
    second: { multiplier: 1, suffix: '/sec' },
    minute: { multiplier: 60, suffix: '/min' },
    hour: { multiplier: 3600, suffix: '/hour' },
    day: { multiplier: 86400, suffix: '/day' },
};

// Per-facility owned tiers: `{ 'Farmland': [{count: 5, level: 3}, {count: 4, level: 5}], ... }`.
// The single source of truth for what's owned; rendering reads FROM this, input edits write
// BACK into it, and `getPlanInputValues()` sends it straight to the solver as-is. A player
// commonly upgrades some but not all of their plots of one facility type (e.g. 5 Farmland at
// level 3 and 4 more upgraded to level 5), so a facility can own more than one tier; facilities
// that don't level up at all (`hasLevels: false`) only ever have exactly one.
let facilityTiers = {};

function defaultFacilityTiers() {
    const tiers = {};
    FACILITIES.forEach(f => {
        tiers[f.name] = [{ count: f.defaultCount, level: 1 }];
    });
    return tiers;
}

// Renders one facility's tier rows (Count + Level inputs, a remove button once there's more than
// one tier, and, only for facilities that level up, an "Add level" button) into its
// `.facility-tiers` container. Called on initial render and again, for just that one facility,
// whenever a tier is added or removed, so editing one facility never disturbs another's inputs.
function renderTierRows(name) {
    const f = FACILITIES.find(fac => fac.name === name);
    const container = document.querySelector(`.facility-tiers[data-facility="${name}"]`);
    if (!f || !container) return;
    const tiers = facilityTiers[name];
    const showRemove = tiers.length > 1;
    container.innerHTML = tiers.map((tier, i) => `
        <div class="facility-inputs tier-row" data-tier-index="${i}">
            <div class="input-field">
                <label>Count</label>
                <input type="number" class="tier-count" value="${tier.count}" min="0" max="999">
            </div>
            ${f.hasLevels === false ? '' : `
            <div class="input-field">
                <label>Level</label>
                <input type="number" class="tier-level" value="${tier.level}" min="1" max="10">
            </div>
            `}
            ${showRemove ? '<button type="button" class="tier-remove-btn" title="Remove this level">&times;</button>' : ''}
        </div>
    `).join('');
}

// Build the facility-card inputs, grouped into a labeled section per category. Runs before other
// DOM setup. Tier-row inputs and buttons are handled via event delegation (see
// `attachFacilityTierHandlers`) rather than per-element listeners, since rows are added/removed
// dynamically after this initial render.
function renderFacilityCards() {
    const grid = document.getElementById('facilities-grid');
    grid.innerHTML = FACILITY_CATEGORIES.map(category => {
        const cards = FACILITIES.filter(f => f.category === category).map(f => `
            <div class="facility-card">
                <h4>${f.name} <span class="info-icon" data-tooltip="${f.tooltip}">?</span></h4>
                <div class="facility-tiers" data-facility="${f.name}"></div>
                ${f.hasLevels === false ? '' : '<button type="button" class="add-tier-btn" data-facility="' + f.name + '">+ Add level</button>'}
            </div>
        `).join('');
        return `
            <div class="facility-category">
                <h4 class="facility-category-title">${category}</h4>
                <div class="facilities-grid">${cards}</div>
            </div>
        `;
    }).join('');
    FACILITIES.forEach(f => renderTierRows(f.name));
}

// Delegated handlers for the facility grid, covering tier rows added/removed after initial
// render: editing a Count/Level input updates `facilityTiers` and persists it; "+ Add level"
// appends a new tier (guessing the next level up from the highest owned, capped at 10); "×"
// removes a tier. Attach once, on the grid container, rather than per-row.
function attachFacilityTierHandlers() {
    const grid = document.getElementById('facilities-grid');

    grid.addEventListener('input', (e) => {
        const row = e.target.closest('.tier-row');
        if (!row) return;
        const container = e.target.closest('.facility-tiers');
        const name = container.dataset.facility;
        const idx = parseInt(row.dataset.tierIndex, 10);
        const tier = facilityTiers[name][idx];
        if (e.target.classList.contains('tier-count')) {
            tier.count = numberOrDefault(e.target.value, 0);
        } else if (e.target.classList.contains('tier-level')) {
            tier.level = numberOrDefault(e.target.value, 1);
        }
        saveInputsToStorage();
    });

    grid.addEventListener('click', (e) => {
        const addBtn = e.target.closest('.add-tier-btn');
        if (addBtn) {
            const name = addBtn.dataset.facility;
            const tiers = facilityTiers[name];
            const nextLevel = Math.min(10, Math.max(...tiers.map(t => t.level)) + 1);
            tiers.push({ count: 1, level: nextLevel });
            renderTierRows(name);
            saveInputsToStorage();
            return;
        }
        const removeBtn = e.target.closest('.tier-remove-btn');
        if (removeBtn) {
            const row = removeBtn.closest('.tier-row');
            const container = removeBtn.closest('.facility-tiers');
            const name = container.dataset.facility;
            const idx = parseInt(row.dataset.tierIndex, 10);
            facilityTiers[name].splice(idx, 1);
            renderTierRows(name);
            saveInputsToStorage();
        }
    });

    // Enter key inside a tier input triggers a full plan recalculation, same as every other
    // input; delegated (rather than the per-input listener loop used for static inputs) since
    // tier inputs come and go as levels are added/removed.
    grid.addEventListener('keypress', (e) => {
        if (e.key === 'Enter' && e.target.matches('input')) {
            runFindPlan();
        }
    });
}

// --- Local persistence -----------------------------------------------------------------
// Saves/restores form inputs via localStorage so values survive a page reload. Purely
// client-side (no account, no server); works identically on localhost and once this is
// hosted on GitHub Pages, since localStorage is scoped to the page's own origin.
const STORAGE_KEY = 'aniimax-config-v1';

// True once the player has picked a rate unit themselves this visit. Until then a fresh plan
// picks the unit it reads best at; after it, their choice stands.
let rateUnitChosen = false;

// Every plain input ID whose value should be persisted (facility tiers are saved separately;
// see `facilityTiers`/`initFacilityTiers`, since they're a dynamic list rather than one fixed
// element per facility).
function getPersistedFieldIds() {
    return [
        'target-amount', 'current-amount',
        'strategy-level-up', 'strategy-priorities', 'level-up-target',
        'mode-simple', 'mode-advanced', 'home-level',
        'ecological-module-level', 'kitchen-module-level',
        'resource-detector-level', 'crafting-module-level',
        'rate-unit'
    ];
}

// Reads and parses the saved config blob, or `null` if there isn't one / it's corrupt.
function readStorage() {
    try {
        const raw = localStorage.getItem(STORAGE_KEY);
        return raw ? migrateSavedConfig(JSON.parse(raw)) : null;
    } catch (e) {
        console.warn('Could not load saved inputs from localStorage:', e);
        return null;
    }
}

// Carries a save made under an older name forward to its current one, so a returning user keeps
// their inputs across a rename instead of silently falling back to defaults. The full release
// renamed Mineral Pile to Mine and the Mineral Detector module to Resource Detector.
function migrateSavedConfig(data) {
    if (!data || typeof data !== 'object') return data;
    // Saves from before simple mode existed hold a hand-entered setup; keep showing it.
    if (data['mode-simple'] === undefined && data.facilityTiers) {
        data['mode-simple'] = false;
        data['mode-advanced'] = true;
    }
    const tiers = data.facilityTiers;
    if (tiers && tiers['Mine'] === undefined && tiers['Mineral Pile'] !== undefined) {
        tiers['Mine'] = tiers['Mineral Pile'];
    }
    if (data['resource-detector-level'] === undefined && data['mineral-detector-level'] !== undefined) {
        data['resource-detector-level'] = data['mineral-detector-level'];
    }
    return data;
}

// Populates the module-level `facilityTiers` from a saved config blob (see `readStorage`),
// falling back to defaults for any facility missing from it; covers both a fresh page load
// (no save yet) and a facility newly added to `FACILITIES` since the user's last save.
function initFacilityTiers(data) {
    const defaults = defaultFacilityTiers();
    const saved = (data && data.facilityTiers) || {};
    facilityTiers = {};
    FACILITIES.forEach(f => {
        const tiers = saved[f.name];
        facilityTiers[f.name] = Array.isArray(tiers) && tiers.length > 0
            ? tiers.map(t => ({
                count: numberOrDefault(t.count, 0),
                level: f.hasLevels === false ? 1 : numberOrDefault(t.level, 1)
            }))
            : defaults[f.name];
    });

}

function saveInputsToStorage() {
    const data = { facilityTiers, levelUpStock, skippedRecipes: [...skippedRecipes], unlockedSpecial: [...unlockedSpecial], priorities: priorityOrder };
    getPersistedFieldIds().forEach(id => {
        const el = document.getElementById(id);
        if (!el) return;
        data[id] = (el.type === 'checkbox' || el.type === 'radio') ? el.checked : el.value;
    });
    try {
        localStorage.setItem(STORAGE_KEY, JSON.stringify(data));
    } catch (e) {
        console.warn('Could not save inputs to localStorage:', e);
    }
}

function loadInputsFromStorage(data) {
    if (!data) return;
    if (data.levelUpStock && typeof data.levelUpStock === 'object') levelUpStock = { ...data.levelUpStock };
    if (Array.isArray(data.skippedRecipes)) skippedRecipes = new Set(data.skippedRecipes.filter(n => typeof n === 'string'));
    if (Array.isArray(data.unlockedSpecial)) unlockedSpecial = new Set(data.unlockedSpecial.filter(n => typeof n === 'string'));
    if (Array.isArray(data.priorities)) {
        const saved = data.priorities.filter(p => PRIORITY_TARGETS.some(t => t.id === p?.target));
        const missing = PRIORITY_TARGETS.filter(t => !saved.some(p => p.target === t.id)).map(t => ({ target: t.id, on: false }));
        priorityOrder = [...saved.map(p => ({ target: p.target, on: !!p.on })), ...missing];
    } else if (data['strategy-coins'] || data['strategy-exp'] || data['strategy-aniipods']) {
        // Saves from before Priorities picked one "Most ..." tab; carry that choice over.
        const first = data['strategy-exp'] ? 'aniimo_exp' : data['strategy-aniipods'] ? 'aniipods' : 'coins';
        priorityOrder = [first, ...PRIORITY_TARGETS.map(t => t.id).filter(id => id !== first)]
            .map(target => ({ target, on: target === first }));
        data['strategy-priorities'] = true;
    }
    getPersistedFieldIds().forEach(id => {
        if (!(id in data)) return;
        const el = document.getElementById(id);
        if (!el) return;
        if (el.type === 'checkbox' || el.type === 'radio') {
            el.checked = !!data[id];
        } else {
            el.value = data[id];
        }
    });
}

// Auto-save on every change to a persisted static field (facility tier inputs save themselves;
// see `attachFacilityTierHandlers`).
function attachAutoSave() {
    getPersistedFieldIds().forEach(id => {
        const el = document.getElementById(id);
        if (!el) return;
        const eventName = (el.type === 'checkbox' || el.type === 'radio' || el.tagName === 'SELECT') ? 'change' : 'input';
        el.addEventListener(eventName, saveInputsToStorage);
    });
}

function clearSavedInputs() {
    try {
        localStorage.removeItem(STORAGE_KEY);
    } catch (e) {
        console.warn('Could not clear saved inputs from localStorage:', e);
    }
    window.location.reload();
}

// Initialize the worker and its wasm module.
async function initWasm() {
    try {
        initWorker();
        const version = await callWorker('get_version');
        wasmReady = true;

        document.getElementById('version').textContent = version;
        loadRecipeIndex();

        console.log(`Aniimax v${version} loaded successfully`);
    } catch (error) {
        console.error('Failed to initialize WASM:', error);
        showError('Failed to load the optimizer. Please refresh the page.');
    }
}

// --- Simple / advanced mode -----------------------------------------------------------
// Simple mode takes just the RV (Homeland) level and assumes everything that level allows is built
// and upgraded (see `simpleSetup` in facility-config.js). Advanced mode is the full per-facility
// input. Switching modes never overwrites the advanced inputs; "Fill from RV level" copies a
// simple setup into them on purpose.

function isSimpleMode() {
    return document.getElementById('mode-simple').checked;
}

function selectedHomeLevel() {
    return numberOrDefault(document.getElementById('home-level').value, MAX_HOME_LEVEL);
}

function populateHomeLevels() {
    const options = [];
    for (let level = 1; level <= MAX_HOME_LEVEL; level++) {
        options.push(`<option value="${level}">${level}${level === MAX_HOME_LEVEL ? ' (everything unlocked)' : ''}</option>`);
    }
    for (const id of ['home-level', 'fill-level']) {
        const select = document.getElementById(id);
        select.innerHTML = options.join('');
        select.value = String(MAX_HOME_LEVEL);
    }
}

// One entry per built facility, e.g. "10 Farmland Lv.2".
function renderSimpleSummary() {
    const homeLevel = selectedHomeLevel();
    const { facilities, modules } = simpleSetup(homeLevel);
    const chip = (count, name, level) => `
        <div class="chip"><span><span class="chip-count">${count}</span> ${name}</span>${level ? `<span class="chip-level">${level}</span>` : ''}</div>`;
    const built = FACILITIES
        .map(f => ({ name: f.name, tier: facilities[f.name][0], hasLevels: f.hasLevels !== false }))
        .filter(({ tier }) => tier.count > 0)
        .map(({ name, tier, hasLevels }) => chip(`${tier.count}×`, name, hasLevels ? `Lv.${tier.level}` : ''))
        .join('');
    const moduleChips = [
        ['Ecological Module', modules.ecological_module],
        ['Kitchen Module', modules.kitchen_module],
        ['Resource Detector', modules.resource_detector],
        ['Crafting Module', modules.crafting_module],
    ].map(([name, level]) => chip('', name, level > 0 ? `Lv.${level}` : 'not yet')).join('');
    const kinds = FACILITIES.filter(f => facilities[f.name][0].count > 0).length;
    document.getElementById('simple-summary-title').textContent = `${kinds} facilities and 4 modules at RV ${homeLevel}`;
    document.getElementById('simple-summary').innerHTML = `
        <p class="assume-title">Facilities</p>
        <div class="chip-grid">${built}</div>
        <p class="assume-title">Modules</p>
        <div class="chip-grid">${moduleChips}</div>`;
}

function applyConfigMode() {
    const simple = isSimpleMode();
    document.getElementById('simple-config').style.display = simple ? 'block' : 'none';
    document.getElementById('advanced-config').style.display = simple ? 'none' : 'block';
    if (simple) renderSimpleSummary();
    renderStrategy();
}

// Fills the advanced inputs with everything `homeLevel` allows.
function fillAdvancedFrom(homeLevel) {
    const { facilities, modules } = simpleSetup(homeLevel);
    FACILITIES.forEach(f => {
        facilityTiers[f.name] = facilities[f.name].map(t => ({ ...t }));
        renderTierRows(f.name);
    });
    document.getElementById('ecological-module-level').value = modules.ecological_module;
    document.getElementById('kitchen-module-level').value = modules.kitchen_module;
    document.getElementById('resource-detector-level').value = modules.resource_detector;
    document.getElementById('crafting-module-level').value = modules.crafting_module;
    saveInputsToStorage();
}

function attachModeHandlers() {
    document.getElementById('mode-simple').addEventListener('change', applyConfigMode);
    document.getElementById('mode-advanced').addEventListener('change', applyConfigMode);
    document.getElementById('home-level').addEventListener('change', () => {
        renderSimpleSummary();
        renderStrategy();
    });
    document.getElementById('fill-btn').addEventListener('click', () => {
        fillAdvancedFrom(numberOrDefault(document.getElementById('fill-level').value, MAX_HOME_LEVEL));
        renderStrategy();
    });
}

// --- Special recipes -------------------------------------------------------------------
// Recipes unlocked with a rare currency (see `SPECIAL_RECIPES`): left out of plans unless ticked.

let unlockedSpecial = new Set();
const SPECIAL_NAMES = new Set(SPECIAL_RECIPES.map(r => r.name));

function renderSpecialRecipes() {
    document.getElementById('special-grid').innerHTML = SPECIAL_RECIPES.map(r => `
        <label class="special-option">
            <input type="checkbox" data-special="${r.name}"${unlockedSpecial.has(r.name) ? ' checked' : ''}>
            <span>${prettyItem(r.name)}</span>
        </label>`).join('');
}

function attachSpecialHandlers() {
    document.getElementById('special-grid').addEventListener('change', (e) => {
        const name = e.target.dataset.special;
        if (!name) return;
        if (e.target.checked) unlockedSpecial.add(name); else unlockedSpecial.delete(name);
        renderRecipeCount();
        saveInputsToStorage();
    });
}

// Every recipe plans may not use: the player's skips and any special recipe not unlocked.
function excludedRecipes() {
    const locked = SPECIAL_RECIPES.map(r => r.name).filter(name => !unlockedSpecial.has(name));
    // Going for Aniipods means the best tier only; the others would be cheaper but catch worse.
    const best = wantsAniipods() ? bestAniipod() : null;
    const lesser = best ? ANIIPOD_TIERS.filter(name => name !== best) : [];
    return [...new Set([...skippedRecipes, ...locked, ...lesser])];
}

// --- Recipes to skip -------------------------------------------------------------------
// Recipes plans may not use (see `JsPlanInput::exclude` in wasm.rs), for things the player can't
// make yet. Saved with the other inputs.

let skippedRecipes = new Set();

// Every recipe as `{ name, facility }`, loaded once for the search box.
let recipeIndex = [];

function recipeLabel(recipe) {
    return `${prettyItem(recipe.name)} (${recipe.facility})`;
}

async function loadRecipeIndex() {
    try {
        recipeIndex = JSON.parse(await callWorker('get_all_items'))
            .map(r => ({ name: r.name, facility: r.facility, cost: r.cost || 0 }))
            .sort((a, b) => a.facility.localeCompare(b.facility) || a.name.localeCompare(b.name));
        document.getElementById('skip-options').innerHTML =
            recipeIndex.map(r => `<option value="${recipeLabel(r)}"></option>`).join('');
        renderSkippedRecipes();
    } catch (error) {
        console.warn('Could not load the recipe list:', error);
    }
}

// The Recipes section's badge, e.g. " (2 on, 3 skipped)", so what's set shows while it's closed.
function renderRecipeCount() {
    const parts = [];
    if (unlockedSpecial.size) parts.push(`${unlockedSpecial.size} on`);
    if (skippedRecipes.size) parts.push(`${skippedRecipes.size} skipped`);
    document.getElementById('recipe-count').textContent = parts.length ? ` (${parts.join(', ')})` : '';
}

function renderSkippedRecipes() {
    renderRecipeCount();
    const facilityOf = name => recipeIndex.find(r => r.name === name)?.facility;
    document.getElementById('skip-chips').innerHTML = [...skippedRecipes]
        .sort((a, b) => prettyItem(a).localeCompare(prettyItem(b)))
        .map(name => {
            const facility = facilityOf(name);
            return `<span class="skip-chip">${prettyItem(name)}${facility ? ` <span class="skip-chip-facility">${facility}</span>` : ''}<button type="button" data-unskip="${name}" aria-label="Stop skipping ${prettyItem(name)}" title="Stop skipping">✕</button></span>`;
        }).join('');
}

function setSkipped(name, skipped) {
    if (skipped) skippedRecipes.add(name); else skippedRecipes.delete(name);
    renderSkippedRecipes();
    saveInputsToStorage();
}

// Adds what's typed in the search box: a full "Item (Facility)" pick, or a unique partial match.
function addSkipFromInput() {
    const input = document.getElementById('skip-input');
    const text = input.value.trim().toLowerCase();
    if (!text) return;
    let match = recipeIndex.find(r => recipeLabel(r).toLowerCase() === text);
    if (!match) {
        const partial = recipeIndex.filter(r => recipeLabel(r).toLowerCase().includes(text));
        if (partial.length === 1) match = partial[0];
    }
    if (!match) {
        input.setCustomValidity('Pick a recipe from the list.');
        input.reportValidity();
        return;
    }
    input.setCustomValidity('');
    input.value = '';
    setSkipped(match.name, true);
}

function attachSkipHandlers() {
    document.getElementById('skip-add-btn').addEventListener('click', addSkipFromInput);
    const input = document.getElementById('skip-input');
    input.addEventListener('keydown', (e) => {
        if (e.key === 'Enter') {
            e.preventDefault();
            addSkipFromInput();
        }
    });
    input.addEventListener('input', () => input.setCustomValidity(''));
    document.getElementById('skip-chips').addEventListener('click', (e) => {
        const name = e.target.closest('[data-unskip]')?.dataset.unskip;
        if (name) setSkipped(name, false);
    });
    // The ✕ on a plan row skips that recipe and plans again.
    document.getElementById('facility-plan-container').addEventListener('click', (e) => {
        const name = e.target.closest('[data-skip]')?.dataset.skip;
        if (!name) return;
        setSkipped(name, true);
        runFindPlan();
    });
}

// --- Strategy ----------------------------------------------------------------------------
// "Level up" plans the soonest next RV level-up (its coins plus Wood Blocks and Mineral Sand, or
// from RV 7 a Woodworking Bench item and a Chimney Kiln item, less what's already in stock);
// "Priorities" makes as much of each ticked priority as the ones above it allow.

// What the player has toward a level-up, by item name ('coins' for coins).
let levelUpStock = {};

const ITEM_NAMES = {
    coins: 'Coins',
    wood_block: 'Wood Blocks',
    mineral_sand: 'Mineral Sand',
    coarse_sifted_ore: 'Coarse-Sifted Ore',
    river_washed_stones: 'River-Washed Stones',
    premium_river_washed_stones: 'Premium River-Washed Stones',
    sugar_roasted_chestnuts: 'Sugar-Roasted Chestnuts',
    flowers_in_a_bottle: 'Flowers in a Bottle',
};

function isLevelUpStrategy() {
    return document.getElementById('strategy-level-up').checked;
}

// --- Priorities ------------------------------------------------------------------------
// What the Priorities strategy can go for, and the player's ranking of it. The plan makes as much
// of each ticked one as the ones above it allow, then earns coins with what's left (see
// `JsPlanInput::priorities` in wasm.rs).
const PRIORITY_TARGETS = [
    { id: 'coins', label: 'Coins' },
    { id: 'aniimo_exp', label: 'Aniimo EXP' },
    { id: 'aniipods', label: 'Aniipods' },
    { id: 'Wood Blocks', label: 'Wood Blocks' },
    { id: 'Mineral Sand', label: 'Mineral Sand' },
];

// Drawn arrows rather than the ↑/↓ characters, which some systems render as colored emoji.
const ARROW_UP = '<svg viewBox="0 0 12 12" width="12" height="12" aria-hidden="true"><path d="M6 2.5 L10 7 H2 Z" fill="currentColor"/></svg>';
const ARROW_DOWN = '<svg viewBox="0 0 12 12" width="12" height="12" aria-hidden="true"><path d="M6 9.5 L10 5 H2 Z" fill="currentColor"/></svg>';

let priorityOrder = PRIORITY_TARGETS.map(t => ({ target: t.id, on: t.id === 'coins' }));

function isPriorityStrategy() {
    return document.getElementById('strategy-priorities').checked;
}

// The ticked priorities, best first; none for the level-up strategy.
function activePriorities() {
    return isPriorityStrategy() ? priorityOrder.filter(p => p.on).map(p => p.target) : [];
}

function wantsAniipods() {
    return activePriorities().includes('aniipods');
}

// A priority's name, with the Aniipod tier the plan would make.
function priorityLabel(target, aniipod = bestAniipod()) {
    if (target === 'aniipods') return aniipod ? prettyItem(aniipod) : 'Aniipods';
    return PRIORITY_TARGETS.find(t => t.id === target)?.label || CURRENCY_LABELS[target] || target;
}

function renderPriorities() {
    const best = bestAniipod();
    document.getElementById('priority-list').innerHTML = priorityOrder.map((p, i) => {
        const label = priorityLabel(p.target, best);
        const note = p.target === 'aniipods' && !best ? ' <span class="hint small">(no Aniipod Maker yet)</span>' : '';
        return `
        <li class="priority${p.on ? '' : ' off'}" draggable="true" data-index="${i}">
            <span class="drag-handle" aria-hidden="true">⋮⋮</span>
            <span class="priority-rank">${p.on ? priorityOrder.slice(0, i + 1).filter(q => q.on).length : ''}</span>
            <span class="priority-name">${label}${note}</span>
            <label class="priority-switch" title="${p.on ? 'On: the plan goes for this' : 'Off: the plan ignores this'}">
                <input type="checkbox" role="switch" data-toggle="${i}" aria-label="${label}"${p.on ? ' checked' : ''}>
                <span class="switch-track" aria-hidden="true"></span>
                <span class="switch-text">${p.on ? 'On' : 'Off'}</span>
            </label>
            <span class="priority-move">
                <button type="button" data-move="${i}" data-by="-1" aria-label="Move ${label} up"${i === 0 ? ' disabled' : ''}>${ARROW_UP}</button>
                <button type="button" data-move="${i}" data-by="1" aria-label="Move ${label} down"${i === priorityOrder.length - 1 ? ' disabled' : ''}>${ARROW_DOWN}</button>
            </span>
        </li>`;
    }).join('');
}

function movePriority(from, to) {
    if (to < 0 || to >= priorityOrder.length || from === to) return;
    const [moved] = priorityOrder.splice(from, 1);
    priorityOrder.splice(to, 0, moved);
    renderPriorities();
    saveInputsToStorage();
}

function attachPriorityHandlers() {
    const list = document.getElementById('priority-list');
    list.addEventListener('change', (e) => {
        const i = e.target.dataset.toggle;
        if (i === undefined) return;
        priorityOrder[i].on = e.target.checked;
        renderPriorities();
        saveInputsToStorage();
    });
    list.addEventListener('click', (e) => {
        const button = e.target.closest('[data-move]');
        if (!button) return;
        const from = Number(button.dataset.move);
        movePriority(from, from + Number(button.dataset.by));
        list.querySelector(`[data-move="${from + Number(button.dataset.by)}"][data-by="${button.dataset.by}"]`)?.focus();
    });
    let dragFrom = null;
    list.addEventListener('dragstart', (e) => {
        const item = e.target.closest('li[data-index]');
        if (!item) return;
        dragFrom = Number(item.dataset.index);
        item.classList.add('dragging');
        e.dataTransfer.effectAllowed = 'move';
        e.dataTransfer.setData('text/plain', String(dragFrom));
    });
    list.addEventListener('dragover', (e) => {
        if (dragFrom === null) return;
        e.preventDefault();
        const over = e.target.closest('li[data-index]');
        list.querySelectorAll('.drop-target').forEach(el => el.classList.remove('drop-target'));
        if (over) over.classList.add('drop-target');
    });
    list.addEventListener('drop', (e) => {
        e.preventDefault();
        const over = e.target.closest('li[data-index]');
        if (dragFrom !== null && over) movePriority(dragFrom, Number(over.dataset.index));
        dragFrom = null;
    });
    list.addEventListener('dragend', () => {
        dragFrom = null;
        renderPriorities();
    });
}

// The best Aniipod the owned Aniipod Maker can make, or null without one. A better Aniipod
// catches better, so the strategy makes only this one.
function bestAniipod() {
    const tiers = isSimpleMode()
        ? simpleSetup(selectedHomeLevel()).facilities['Aniipod Maker']
        : facilityTiers['Aniipod Maker'];
    const level = Math.max(0, ...(tiers || []).filter(t => t.count > 0).map(t => t.level));
    return level > 0 ? ANIIPOD_TIERS[Math.min(level, ANIIPOD_TIERS.length) - 1] : null;
}

// The RV level being worked toward: the next one in simple mode, the picked one in advanced.
function levelUpTarget() {
    if (isSimpleMode()) return selectedHomeLevel() + 1;
    return numberOrDefault(document.getElementById('level-up-target').value, 7);
}

// The target's cost, or null if it isn't known.
function levelUpCost() {
    return LEVEL_UP_COSTS[levelUpTarget()] || null;
}

// Why the level-up can't be planned, or null if it can.
function levelUpUnavailable() {
    const target = levelUpTarget();
    if (target > MAX_HOME_LEVEL) return `RV ${MAX_HOME_LEVEL} is the top level, so there's no level-up to plan.`;
    if (!LEVEL_UP_COSTS[target]) return `There's no level-up cost for RV ${target}.`;
    return null;
}

// Everything worth counting toward `cost`: coins, and each chain up to the tier it needs.
function stockNames(cost) {
    const names = ['coins'];
    cost.items.forEach(([item]) => {
        const chain = LEVEL_UP_CHAINS.find(c => c.includes(item));
        if (chain) names.push(...chain.slice(0, chain.indexOf(item) + 1));
    });
    return names;
}

function stockAmount(name) {
    const amount = Number(levelUpStock[name]);
    return Number.isFinite(amount) && amount > 0 ? amount : 0;
}

function populateLevelUpTargets() {
    const select = document.getElementById('level-up-target');
    select.innerHTML = Object.keys(LEVEL_UP_COSTS).map(level => `<option value="${level}">${level}</option>`).join('');
}

function renderStrategy() {
    const levelUp = isLevelUpStrategy();
    document.getElementById('level-up-config').style.display = levelUp ? 'block' : 'none';
    document.getElementById('priorities-config').style.display = levelUp ? 'none' : 'block';
    if (!levelUp) {
        renderPriorities();
        return;
    }

    // Simple mode always plans the next RV level, so only Advanced picks one.
    document.getElementById('level-up-target-row').style.display = isSimpleMode() ? 'none' : '';

    const costEl = document.getElementById('level-up-cost');
    const stockDetails = document.getElementById('level-up-stock');
    const unavailable = levelUpUnavailable();
    if (unavailable) {
        costEl.innerHTML = `<p class="level-up-note">${unavailable} Plans will go for the most coins.</p>`;
        stockDetails.style.display = 'none';
        return;
    }
    const cost = levelUpCost();
    const chip = (amount, name) => `<div class="chip"><span><span class="chip-count">${formatNumber(amount)}</span> ${ITEM_NAMES[name] || prettyItem(name)}</span></div>`;
    costEl.innerHTML = `
        <p class="assume-title">RV ${levelUpTarget()} costs</p>
        <div class="chip-grid">${chip(cost.coins, 'coins')}${cost.items.map(([item, n]) => chip(n, item)).join('')}</div>`;
    stockDetails.style.display = '';
    document.getElementById('level-up-stock-grid').innerHTML = stockNames(cost).map(name => `
        <div class="input-field">
            <label for="stock-${name}">${ITEM_NAMES[name] || prettyItem(name)}</label>
            <input type="number" id="stock-${name}" data-stock="${name}" min="0" value="${stockAmount(name)}">
        </div>`).join('');
}

function attachStrategyHandlers() {
    document.getElementById('strategy-level-up').addEventListener('change', renderStrategy);
    document.getElementById('strategy-priorities').addEventListener('change', renderStrategy);
    document.getElementById('level-up-target').addEventListener('change', renderStrategy);
    const grid = document.getElementById('level-up-stock-grid');
    grid.addEventListener('input', (e) => {
        const name = e.target.dataset.stock;
        if (!name) return;
        levelUpStock[name] = Math.max(0, floatOrDefault(e.target.value, 0));
        saveInputsToStorage();
    });
    grid.addEventListener('keypress', (e) => {
        if (e.key === 'Enter' && e.target.matches('input')) runFindPlan();
    });
}

// The level-up the solver should plan for (see `JsPlanInput::level_up` in wasm.rs), or null.
function levelUpInput() {
    if (!isLevelUpStrategy() || levelUpUnavailable()) return null;
    const cost = levelUpCost();
    return {
        cost: [['coins', cost.coins], ...cost.items],
        stock: stockNames(cost).filter(name => stockAmount(name) > 0).map(name => [name, stockAmount(name)]),
    };
}

// A per-second rate as a per-hour figure, with a decimal when it's small.
function perHour(perSecond) {
    const hourly = perSecond * 3600;
    return hourly < 10 ? hourly.toFixed(1) : formatNumber(Math.round(hourly));
}

// "2d 4h", "5h 12m", "12m": how long until a level-up is covered.
function formatDuration(seconds) {
    const minutes = Math.ceil(seconds / 60);
    const days = Math.floor(minutes / 1440);
    const hours = Math.floor((minutes % 1440) / 60);
    const mins = minutes % 60;
    if (days > 0) return `${days}d ${hours}h`;
    if (hours > 0) return `${hours}h ${mins}m`;
    return `${mins}m`;
}

// What the plans on screen were asked for, so they're described against the right target even
// after the inputs change.
let planContext = null;

// The level-up part of the rate card: how soon the plan covers the target's cost, one line per
// cost at the selected rate unit. With a table to show, the unit select moves into its rate
// column and the separate coin rate is hidden, since the Coins row says the same.
function renderLevelUp(plan) {
    const card = document.getElementById('level-up-card');
    const select = document.getElementById('rate-unit');
    const rateBlock = document.getElementById('rate-block');
    if (select.closest('#level-up-card')) document.getElementById('rate-line').appendChild(select);
    rateBlock.style.display = '';
    const context = planContext;
    if (!context || !context.levelUp) {
        card.style.display = 'none';
        return;
    }
    card.style.display = 'block';
    const label = document.getElementById('level-up-label');
    const time = document.getElementById('level-up-time');
    const lines = document.getElementById('level-up-lines');
    label.textContent = `RV ${context.target} level-up`;
    const report = plan.level_up;
    if (context.unavailable) {
        time.textContent = '-';
        lines.innerHTML = `<p class="level-up-note">${context.unavailable} This plan is for the most coins.</p>`;
        return;
    }
    if (context.ready) {
        time.textContent = 'Ready now';
        lines.innerHTML = `<p class="level-up-note">You already have everything it costs. This plan is for the most coins.</p>`;
        return;
    }
    if (!report) {
        const why = plan.level_up_note === 'unreachable'
            ? `These facilities can't make everything it costs.`
            : `The level-up couldn't be planned.`;
        time.textContent = '-';
        lines.innerHTML = `<p class="level-up-note">${why} This plan is for the most coins.</p>`;
        return;
    }
    time.textContent = `in ${formatDuration(report.seconds)}`;
    const { multiplier } = RATE_UNIT_SECONDS[select.value] || RATE_UNIT_SECONDS.second;
    const perUnit = perSecond => formatRate(perSecond * multiplier);
    const slowest = Math.max(...report.requirements.map(r => r.seconds ?? Infinity));
    const rows = report.requirements.map(r => {
        const ready = r.seconds === null ? 'never' : r.seconds === 0 ? 'have it' : formatDuration(r.seconds);
        const isSlowest = r.seconds !== null && r.seconds > 0 && r.seconds >= slowest * (1 - 1e-6);
        return `<tr${isSlowest ? ' class="slowest"' : ''}>
            <td>${ITEM_NAMES[r.name] || prettyItem(r.name)}</td>
            <td>${formatNumber(r.need)}</td>
            <td>${formatNumber(r.have)}</td>
            <td>${perUnit(r.per_second)}</td>
            <td>${ready}</td>
        </tr>`;
    }).join('');
    // What's left over once everything is ready and paid for: costs that finish early keep coming
    // in while the slowest one finishes.
    const surplus = report.requirements
        .map(r => ({ name: r.name, spare: Math.floor(r.have + r.per_second * report.seconds - r.need) }))
        .concat((report.leftovers || []).map(([name, amount]) => ({ name, spare: Math.floor(amount) })))
        .filter(r => r.spare >= 1)
        .map(r => `${formatNumber(r.spare)} ${r.name === 'coins' ? 'coins' : ITEM_NAMES[r.name] || prettyItem(r.name)}`);
    const coinsNote = surplus.length
        ? `<p class="level-up-coins"><span>Surplus:</span> <strong>${surplus.join(', ')}</strong></p>`
        : '';
    lines.innerHTML = `
        <table class="level-up-lines">
            <thead><tr><th>Cost</th><th>Need</th><th>Have</th><th id="level-up-rate-head"></th><th>Ready in</th></tr></thead>
            <tbody>${rows}</tbody>
        </table>
        ${coinsNote}`;
    document.getElementById('level-up-rate-head').appendChild(select);
    rateBlock.style.display = 'none';
}

// The seeds a plan plants: one seed per planting of each Farmland and Woodland crop, and what
// they cost. A level-up plan counts them until the level-up is ready; others per the rate
// card's unit. Mines, Wells and resident facilities aren't planted.
function renderSeedTable(plan) {
    const card = document.getElementById('seed-card');
    const el = document.getElementById('seed-table');
    const unit = document.getElementById('rate-unit').value;
    const levelUp = plan.level_up && plan.level_up.seconds > 0 && planContext?.levelUp ? plan.level_up : null;
    const multiplier = levelUp ? levelUp.seconds : (RATE_UNIT_SECONDS[unit] || RATE_UNIT_SECONDS.second).multiplier;
    const rows = (plan.coin_items || [])
        .filter(s => (s.facility === 'Farmland' || s.facility === 'Woodland') && s.status === 'producing' && s.cycle_time > 0)
        .map(s => {
            const perSecond = s.facility_count / s.cycle_time;
            const cost = recipeIndex.find(r => r.name === s.item_name)?.cost || 0;
            // Whole seeds when counting to the level-up.
            const seeds = levelUp ? Math.ceil(perSecond * multiplier) : perSecond * multiplier;
            return { name: s.item_name, facility: s.facility, plots: s.facility_count, seeds, cost: seeds * cost };
        })
        .sort((a, b) => b.seeds - a.seeds);
    if (rows.length === 0) {
        card.style.display = 'none';
        return;
    }
    const amount = formatRate;
    const totalCost = rows.reduce((sum, r) => sum + r.cost, 0);
    card.style.display = 'block';
    const per = levelUp
        ? `until RV ${planContext.target}`
        : { second: 'per second', minute: 'per minute', hour: 'per hour', day: 'per day' }[unit] || 'per second';
    document.getElementById('seed-card-unit').textContent = 'One seed per planting, for every Farmland and Woodland crop in the plan.';
    el.innerHTML = `
        <table>
            <thead><tr><th>Crop</th><th>Plots</th><th>Seeds ${per}</th><th>Cost ${per}</th></tr></thead>
            <tbody>${rows.map(r => `<tr>
                <td>${prettyItem(r.name)}</td>
                <td>${r.plots}</td>
                <td>${amount(r.seeds)}</td>
                <td>${r.cost > 0 ? `${amount(r.cost)} coins` : 'free'}</td>
            </tr>`).join('')}</tbody>
            ${rows.length > 1 && totalCost > 0 ? `<tfoot><tr><td colspan="3">Total</td><td>${amount(totalCost)} coins</td></tr></tfoot>` : ''}
        </table>`;
}

// What each product sold earns in a level-up plan, per hour and by the time the level-up is
// ready. (Priorities plans show this in the goal card instead.)
function renderProfitBreakdown(plan) {
    const card = document.getElementById('profit-card');
    const report = plan.level_up;
    const streams = (plan.income_streams || []).filter(s => s.units_per_second > 0);
    if (!report || streams.length === 0) {
        card.style.display = 'none';
        return;
    }
    card.style.display = 'block';
    const total = streams.reduce((sum, s) => sum + s.rate_per_second, 0);
    const rows = [...streams]
        .sort((a, b) => b.rate_per_second - a.rate_per_second)
        .map(s => `<tr>
            <td data-label="Product">${prettyItem(s.item_name)}</td>
            <td data-label="Facility">${s.facility}</td>
            <td data-label="Sold per hour">${perHour(s.units_per_second)}</td>
            <td data-label="Profit per hour">${formatNumber(Math.round(s.rate_per_second * 3600))}</td>
            <td data-label="Share">${total > 0 ? Math.round(s.rate_per_second / total * 100) : 0}%</td>
            <td data-label="Profit until RV ${planContext?.target}">${formatNumber(Math.floor(s.rate_per_second * report.seconds))}</td>
        </tr>`).join('');
    document.getElementById('profit-breakdown').innerHTML = `
        <div class="table-wrapper">
            <table class="facility-plan-table">
                <thead><tr><th>Product</th><th>Facility</th><th>Sold per hour</th><th>Profit per hour</th><th>Share</th><th>Profit until RV ${planContext?.target}</th></tr></thead>
                <tbody>${rows}</tbody>
            </table>
        </div>`;
}

// Get plan-level input values from the form (facilities/modules/prioritize-byproducts, nothing
// goal-related, since find_plan doesn't need a target). Currency is always coins: the full
// release removed Bud Tickets, the only other sellable currency.
function getPlanInputValues() {
    // `facilityTiers` is the live source of truth for owned counts (kept in sync with the DOM by
    // `attachFacilityTierHandlers`), sent straight through as a list of tiers per facility; see
    // `JsPlanInput::facilities` in wasm.rs for the shape (`[{count, level}, ...]` per facility).
    if (isSimpleMode()) {
        const { facilities, modules } = simpleSetup(selectedHomeLevel());
        return {
            currency: 'coins',
            priorities: activePriorities(),
            prioritize_byproducts: false,
            level_up: levelUpInput(),
            exclude: excludedRecipes(),
            facilities,
            modules
        };
    }

    const facilities = {};
    FACILITIES.forEach(f => {
        facilities[f.name] = facilityTiers[f.name].map(t => ({
            count: t.count,
            level: f.hasLevels === false ? 1 : t.level
        }));
    });

    const modules = {
        ecological_module: numberOrDefault(document.getElementById('ecological-module-level').value, 0),
        kitchen_module: numberOrDefault(document.getElementById('kitchen-module-level').value, 0),
        resource_detector: numberOrDefault(document.getElementById('resource-detector-level').value, 0),
        crafting_module: numberOrDefault(document.getElementById('crafting-module-level').value, 0)
    };

    return {
        currency: 'coins',
        priorities: activePriorities(),
        prioritize_byproducts: false,
        level_up: levelUpInput(),
        exclude: excludedRecipes(),
        facilities,
        modules
    };
}

// parseInt/parseFloat that fall back to `fallback` only when the input doesn't parse to a number
// at all (blank/invalid); unlike `value || fallback`, these correctly keep a legitimate 0 (e.g.
// "I own zero of this facility"), which `||` would silently discard since 0 is falsy in JS.
// "quick_aromathyst" -> "Quick Aromathyst": the data uses snake_case names.
function prettyItem(name) {
    if (!name) return name;
    if (ITEM_NAMES[name]) return ITEM_NAMES[name];
    return name.split('_').map(w => w ? w[0].toUpperCase() + w.slice(1) : w).join(' ');
}

// A plan row's reason with its item names made readable: "Used for dried_strawberries, jam; the
// rest sells directly" -> "Used for Dried Strawberries, Jam; the rest sells directly".
function prettyReason(reason) {
    if (!reason) return reason;
    const names = list => list.split(', ').map(prettyItem).join(', ');
    return reason
        .replace(/^Used for ([^;]+)/, (_, list) => 'Used for ' + names(list))
        .replace(/takes turns with ([^;]+)$/, (_, list) => 'takes turns with ' + names(list));
}

// Keys ("Facility|item") of the shown plan's rows that rely on recipes not yet checked in game;
// set by `displayPlan` so the facility tables can tag those rows.
let unverifiedRowKeys = new Set();

function numberOrDefault(value, fallback) {
    const parsed = parseInt(value, 10);
    return Number.isNaN(parsed) ? fallback : parsed;
}

function floatOrDefault(value, fallback) {
    const parsed = parseFloat(value);
    return Number.isNaN(parsed) ? fallback : parsed;
}

// Format number with commas
function formatNumber(num) {
    return num.toLocaleString(undefined, { maximumFractionDigits: 2 });
}

// A rate at the chosen unit: a whole number once it's big enough to read that way, otherwise two
// significant figures, so a slow trickle doesn't round away to zero.
function formatRate(n) {
    return n >= 10 ? formatNumber(Math.round(n)) : String(Number(n.toPrecision(2)));
}

// Show an error in the results section (plan-level failures only; goal-level failures are rare
// and shown inline in the goal section instead, since the plan above it is still valid).
function showError(message) {
    const errorEl = document.getElementById('error-message');
    const resultsContent = document.getElementById('results-content');
    const resultsSection = document.getElementById('results-section');

    errorEl.textContent = message;
    errorEl.style.display = 'block';
    resultsContent.style.display = 'none';
    resultsSection.style.display = 'block';
}

// The goal card's choices: coins and every priority that's on, in the player's order. Keeps the
// current choice when the new plan still has it.
function renderGoalTargets(plan) {
    const select = document.getElementById('goal-target');
    const previous = select.value;
    const rows = priorityRows(plan);
    select.innerHTML = rows.map(r => `<option value="${r.target}">${goalName(r)}</option>`).join('');
    if (rows.some(r => r.target === previous)) select.value = previous;
}

function goalName(row) {
    return row.target === 'coins' ? 'Coins' : row.label;
}

// Renders the item-level production breakdown from `goalResult.products`; one row per income
// stream (a selected item, or the leftover-capacity portion of a split facility), already
// sorted by net profit descending by the solver. Wood Blocks/Mineral Sand byproducts
// (`goalResult.byproducts`) are appended as extra rows at the bottom, styled distinctly since
// they're a side effect of the plan above rather than something sold for the chosen currency.
// The Profit column scales with whichever unit is selected in `#rate-unit` (see
// `updateRateDisplay`), same as "Your Rate" above.
function renderProductBreakdown(goalResult) {
    const section = document.getElementById('product-breakdown-section');
    const tbody = document.getElementById('product-breakdown-tbody');

    const products = goalResult.products || [];
    const byproducts = (goalResult.byproducts || []).filter(([, amount]) => Math.floor(amount) > 0);
    if (products.length === 0 && byproducts.length === 0) {
        section.style.display = 'none';
        return;
    }
    section.style.display = 'block';

    const unit = document.getElementById('rate-unit').value;
    const { multiplier, suffix } = RATE_UNIT_SECONDS[unit] || RATE_UNIT_SECONDS.second;
    document.getElementById('product-breakdown-rate-header').textContent = `Profit${suffix}`;

    tbody.innerHTML = '';
    products.forEach(p => {
        const row = document.createElement('tr');
        // Amount is floored to a whole number; the underlying rate math is a continuous
        // approximation (same steady-state model used throughout this calculator), but you
        // can't actually receive a fractional item; whatever fraction is left over represents
        // a batch still in progress at the moment the goal is reached. Worth is then computed
        // from THAT same whole number (amount * sell price), not the unrounded rate total, so
        // the two columns always reconcile by hand-multiplication; Profit stays net of
        // ingredient costs (matches Total Time/Amount Produced above), so it won't equal Worth
        // / time; they're intentionally different figures (gross vs. net).
        const wholeAmount = Math.floor(p.total_units);
        const worth = wholeAmount * p.sell_value;
        row.innerHTML = `
            <td>${prettyItem(p.item_name)}</td>
            <td>${p.facility}</td>
            <td>${wholeAmount.toLocaleString()}</td>
            <td>${formatRate(p.rate_per_second * multiplier)}</td>
            <td>${formatNumber(worth)}</td>
        `;
        tbody.appendChild(row);
    });

    byproducts.forEach(([name, amount]) => {
        const row = document.createElement('tr');
        row.className = 'byproduct-row';
        row.innerHTML = `
            <td>${name} <span class="hint small">(bonus)</span></td>
            <td>&mdash;</td>
            <td>${Math.floor(amount).toLocaleString()}</td>
            <td>&mdash;</td>
            <td>not sold</td>
        `;
        tbody.appendChild(row);
    });
}

// Renders `goalResult.seed_requirements`; one row per grower crop actually being planted, how
// many times each of its dedicated plots needs replanting over the goal's total time, so a
// player can have enough seeds ready ahead of time. Never includes processor facilities; they
// aren't planted (see `SeedRequirement` in models.rs).
function renderSeedsNeeded(goalResult) {
    const section = document.getElementById('seeds-needed-section');
    const tbody = document.getElementById('seeds-needed-tbody');

    const requirements = goalResult.seed_requirements || [];
    if (requirements.length === 0) {
        section.style.display = 'none';
        return;
    }
    section.style.display = 'block';

    tbody.innerHTML = requirements.map(r => `
        <tr>
            <td>${prettyItem(r.item_name)}</td>
            <td>${r.facility}</td>
            <td>${r.facility_count.toLocaleString()}</td>
            <td>${r.seeds_per_plot.toLocaleString()}</td>
            <td>${r.total_seeds.toLocaleString()}</td>
        </tr>
    `).join('');
}

// Fixed display order for environment groups; matches ENVIRONMENT_BUILDINGS's mode order in
// optimizer.rs (Heat Furnace's two modes, then Cooling Unit's two, then Sunlamp's one).
const ENVIRONMENT_MODE_ORDER = ['Warm', 'Scorching', 'Cool', 'Freeze', 'Adequate'];

// "Fire Lv.3 · Practical" for a row that needs a specific Aniimo, or '-' when it doesn't (crops,
// trees, idle facilities).
// Every Aniimo ability in the game's own order, with its in-game color and what it's for.
// `dark` marks colors light enough to need dark text.
const ABILITIES = [
    { name: 'Fire', color: '#e5484d', about: 'Cooking, smelting and heat' },
    { name: 'Grass', color: '#3fa36b', about: 'Planting seeds and gathering' },
    { name: 'Water', color: '#2b8fe8', about: 'Brewing, fetching water and watering' },
    { name: 'Earth', color: '#b39a74', about: 'Reclaiming land and mining' },
    { name: 'Lightning', color: '#e6c317', about: 'Electricity', dark: true },
    { name: 'Ice', color: '#45c4de', about: 'Cooling the homeland' },
    { name: 'Wind', color: '#2fbfa5', about: 'Processing with wind' },
    { name: 'Dark', color: '#7d4bb3', about: 'Harvesting, cutting, pickling and drying' },
    { name: 'Light', color: '#f5a524', about: 'Lighting the homeland', dark: true },
    { name: 'Hauling', color: '#5f7fd1', about: 'Carrying produce to storage' },
    { name: 'Artisanship', color: '#5fb14f', about: 'Handcrafted goods' },
    { name: 'Leisure', color: '#e8678a', about: 'Making things while playing' },
    { name: 'Perfumery', color: '#b877d9', about: 'Perfumes and incense' },
];
const ABILITY_BY_NAME = new Map(ABILITIES.map(a => [a.name, a]));

// The ability each environment building's Aniimo needs (confirmed in game).
const ENVIRONMENT_BUILDING_ABILITY = {
    'Heat Furnace': 'Fire',
    'Cooling Unit': 'Ice',
    'Sunlamp': 'Light',
};

// A colored ability tag, like the game's.
function abilityTag(name) {
    const a = ABILITY_BY_NAME.get(name);
    if (!a) return name;
    return `<span class="ability${a.dark ? ' dark' : ''}" style="--ability:${a.color}" title="${a.about}">${name}</span>`;
}

// A colored circle with the Aniimo level in it, for the facility plan's Aniimo column; the
// tooltip has the ability, level and personality.
function abilityDot(name, level, note) {
    const a = ABILITY_BY_NAME.get(name);
    const color = a ? a.color : '#888888';
    const tip = `${name} Lv.${level}${note ? ` · ${note}` : ''}`;
    return `<span class="ability-dot${a && a.dark ? ' dark' : ''}${note ? ' bonus' : ''}" style="--ability:${color}" title="${tip}" aria-label="${tip}">${level}</span>`;
}

function aniimoLabel(step) {
    const a = step.aniimo;
    if (!a) {
        // Crops and trees: the abilities their planting and harvesting jobs need.
        const tasks = step.aniimo_tasks || [];
        if (tasks.length === 0) return '-';
        return `<span class="ability-dots">${tasks.map(t => abilityDot(t.ability, t.level)).join('')}</span>`;
    }
    let note = '';
    if (a.personality_bonus) {
        const personality = FACILITIES.find(f => f.name === step.facility)?.personality;
        note = `${personality ? `${personality} personality` : 'matching personality'} (+20% speed)`;
    }
    return `<span class="ability-dots">${abilityDot(a.ability, a.level, note)}</span>`;
}

// "Fire Lv.3 · Practical": one kind of Aniimo, with the facility's personality when the plan
// counts on its bonus. `tagged` shows the ability as a colored tag.
function taskLabel(task, facility, tagged = false) {
    const ability = tagged ? abilityTag(task.ability) : task.ability;
    if (!task.personality_bonus) return `${ability} Lv.${task.level}`;
    const personality = FACILITIES.find(f => f.name === facility)?.personality;
    return `${ability} Lv.${task.level} · ${personality || 'matching personality'}`;
}

function facilityPlanTable(rows) {
    return facilityPlanTableOf([{ rows }]);
}

// One table over several labelled groups, e.g. a paired environment's three zones: each group's
// rows follow a band naming it, so the column headers are written once.
function facilityPlanTableOf(groups) {
    const body = groups
        .map(group => (group.label ? `<tr class="facility-plan-group"><td colspan="5">${group.label}</td></tr>` : '') + planRows(group.rows))
        .join('');
    return `
        <div class="table-wrapper">
            <table class="facility-plan-table">
                <thead>
                    <tr>
                        <th>Facility</th>
                        <th>Count</th>
                        <th>Producing</th>
                        <th>Aniimo</th>
                        <th>Why</th>
                    </tr>
                </thead>
                <tbody>${body}</tbody>
            </table>
        </div>
    `;
}

function planRows(rows) {
    return rows.map(step => `
                    <tr class="status-${step.status}">
                        <td data-label="Facility">${step.facility}</td>
                        <td data-label="Count">${step.facility_count}</td>
                        <td data-label="Producing">${step.item_name ? prettyItem(step.item_name) : '-'}${unverifiedRowKeys.has(`${step.facility}|${step.item_name}`) ? '<span class="tag unverified" title="Not yet checked in game">unverified</span>' : ''}${step.item_name && step.status === 'producing' ? `<button type="button" class="skip-row" data-skip="${step.item_name}" title="Can't make this? Skip it and plan again" aria-label="Skip ${prettyItem(step.item_name)} and plan again">✕</button>` : ''}</td>
                        <td data-label="Aniimo">${aniimoLabel(step)}</td>
                        <td data-label="Why">${prettyReason(step.reason)}</td>
                    </tr>
                `).join('');
}

// "Sowing", "Sowing and Collecting", "Reaping, Logging and Collecting".
function listOf(items) {
    if (items.length < 2) return items.join('');
    return `${items.slice(0, -1).join(', ')} and ${items[items.length - 1]}`;
}

// Where to put the second building. The solver's offset runs corner to corner, so for two 2x2
// buildings in a row the clear space between them is two tiles less; a diagonal one is easier to
// follow as the corner position itself, which is also what the diagram draws.
function gapText(dx, dy = 0) {
    const tiles = n => `${n} tile${n === 1 ? '' : 's'}`;
    if (dy === 0) {
        const between = dx - 2;
        return between <= 0 ? 'Side by side, touching.' : `In a row, ${tiles(between)} between them.`;
    }
    return `Corner to corner, the second sits ${tiles(dx)} along and ${tiles(dy)} up from the first.`;
}


// A growing environment named in its own colour.
function modeTag(mode) {
    const tint = ENVIRONMENT_MODE_COLORS[mode] || '#9aa0a8';
    return `<span class="env-mode-tag" style="--tint:${tint}">${mode}</span>`;
}

// What a row's Aniimo work on: each growing job ("Sowing crops" when it covers both Farmland and
// Woodland, "Reaping farmland" when it's one facility's), then the facilities.
function whereText(g) {
    return [...g.jobs]
        .map(([job, at]) => `${job} ${at.size > 1 ? 'crops' : listOf([...at].map(facility => facility.toLowerCase()))}`)
        .concat([...g.where.entries()].map(([place, n]) => `${n > 1 ? n + '× ' : ''}${place}`))
        .join(', ');
}

// The Aniimo team the shown plan needs, one row per distinct ability / level / personality.
// A row asks for enough Aniimo to cover its work on average (a facility waiting on ingredients
// frees its Aniimo), rounded up; facilities with a resident Aniimo (Sandcastle and the like) are
// always busy, so they count one each.
function renderAniimoSummary(plan) {
    const container = document.getElementById('aniimo-summary');
    const groups = new Map();
    (plan.coin_items || []).forEach(step => {
        (step.aniimo_tasks || []).forEach(task => {
            const key = taskLabel(task, step.facility);
            if (!groups.has(key)) {
                groups.set(key, { label: key, ability: task.ability, level: task.level, bonus: task.personality_bonus, busy: 0, where: new Map(), jobs: new Map() });
            }
            const g = groups.get(key);
            g.busy += task.busy;
            // A growing job reads as the job itself, however many crops it covers.
            if ((task.jobs || []).length) {
                // Where a job happens matters: reaping is Farmland's, logging Woodland's.
                task.jobs.forEach(job => {
                    if (!g.jobs.has(job)) g.jobs.set(job, new Set());
                    g.jobs.get(job).add(step.facility);
                });
                return;
            }
            const place = `${step.facility} (${prettyItem(step.item_name)})`;
            g.where.set(place, (g.where.get(place) || 0) + step.facility_count);
        });
    });
    // Environment buildings in use each keep an Aniimo busy (abilities confirmed in game; whether
    // level or personality matters isn't known yet, so any level is shown).
    const needsAniimo = (building, units, place) => {
        const ability = ENVIRONMENT_BUILDING_ABILITY[building];
        if (!ability || !units) return;
        const key = `${ability} (environment)`;
        if (!groups.has(key)) {
            groups.set(key, { label: `${ability} any level`, ability, level: 1, bonus: false, busy: 0, where: new Map(), jobs: new Map(), environment: true });
        }
        const g = groups.get(key);
        g.busy += units;
        g.where.set(place, (g.where.get(place) || 0) + units);
    };
    // Two buildings placed to overlap report a zone each, all named after the first of them, so
    // count the pair once and give each building its own Aniimo.
    const pairUnits = new Map();
    (plan.environment_assignments || []).forEach(a => {
        if (a.partner) {
            const key = `${a.building}|${a.partner[0]}`;
            const seen = pairUnits.get(key);
            pairUnits.set(key, { units: Math.max(seen ? seen.units : 0, a.units), modes: a.pair_modes || [a.mode, a.mode] });
            return;
        }
        needsAniimo(a.building, a.units, `${a.building} (${a.mode})`);
    });
    pairUnits.forEach(({ units, modes }, key) => {
        const [building, partner] = key.split('|');
        needsAniimo(building, units, `${building} (${modes[0]})`);
        needsAniimo(partner, units, `${partner} (${modes[1]})`);
    });
    const collapsedSummary = document.getElementById('aniimo-collapsed-summary');
    if (groups.size === 0) {
        container.innerHTML = '<p class="hint">Nothing in this plan needs an Aniimo.</p>';
        collapsedSummary.textContent = 'No Aniimo needed.';
        document.getElementById('aniimo-abilities').innerHTML = '';
        return;
    }
    // Each row gets its own Aniimo, which is the clearer team to keep. Only when that asks for
    // more than the homeland holds does work that takes any level (the Farmland and Woodland jobs)
    // move into another row's spare time, so a plot's watering is a job of its own where there's
    // room for one. Rows that count on a personality bonus never merge.
    const assign = share => {
        const rows = [...groups.values()]
            .map(g => ({ ...g, where: new Map(g.where), jobs: new Map([...g.jobs].map(([job, at]) => [job, new Set(at)])) }))
            .sort((a, b) => b.level - a.level || Number(b.bonus) - Number(a.bonus) || a.label.localeCompare(b.label));
        const kept = [];
        rows.forEach(g => {
            const host = share && !g.bonus && !g.environment
                ? kept.find(k => k.ability === g.ability && k.level >= g.level && k.spare >= g.busy - 1e-6)
                : null;
            if (host) {
                host.spare -= g.busy;
                host.busy += g.busy;
                g.where.forEach((n, place) => host.where.set(place, (host.where.get(place) || 0) + n));
                g.jobs.forEach((at, job) => {
                    if (!host.jobs.has(job)) host.jobs.set(job, new Set());
                    at.forEach(facility => host.jobs.get(job).add(facility));
                });
                return;
            }
            g.count = Math.max(1, Math.ceil(g.busy - 1e-6));
            g.spare = g.count - g.busy;
            kept.push(g);
        });
        return { kept, total: kept.reduce((sum, g) => sum + g.count, 1) }; // 1 for the Hauling row
    };
    const homelandHolds = isSimpleMode() ? ANIIMO_MAX[selectedHomeLevel() - 1] : null;
    const own = assign(false);
    const { kept, total: assigned } = homelandHolds && own.total > homelandHolds ? assign(true) : own;
    let total = assigned - kept.reduce((sum, g) => sum + g.count, 0); // the Hauling row
    const rows = kept
        .sort((a, b) => a.label.localeCompare(b.label))
        .map(g => {
            total += g.count;
            const where = whereText(g);
            const rest = g.label.slice(g.ability.length).trim();
            return `<tr><td data-label="Aniimo">${abilityTag(g.ability)} ${rest}</td><td data-label="How many">${g.count}</td><td data-label="Busy on average">${g.busy.toFixed(1)}</td><td data-label="Where">${where}</td></tr>`;
        })
        .join('');
    const haulingRow = `<tr><td data-label="Aniimo">${abilityTag('Hauling')} any level</td><td data-label="How many">1+</td><td data-label="Busy on average">?</td><td data-label="Where">Carries produce to storage. How much work this is isn't known yet; add more if produce piles up.</td></tr>`;

    let capNote = '';
    const cap = homelandHolds;
    if (cap && total > cap) {
        capNote = `<p class="hint small">That's ${total} Aniimo, more than the ${cap} an RV level ${selectedHomeLevel()} homeland holds. Aniimo with more than one of these abilities can cover several rows.</p>`;
    } else if (cap) {
        capNote = `<p class="hint small">That's ${total} Aniimo; an RV level ${selectedHomeLevel()} homeland holds ${cap}.</p>`;
    } else {
        capNote = `<p class="hint small">That's ${total} Aniimo at most; ones with more than one of these abilities can cover several rows.</p>`;
    }
    collapsedSummary.textContent = cap
        ? `${total} Aniimo · your homeland holds ${cap}${total > cap ? ' (too many; see the list)' : ''}`
        : `${total} Aniimo at most`;
    // How many of each ability the plan needs, in the game's order, like its Abilities screen.
    const needed = new Map(ABILITIES.map(a => [a.name, 0]));
    kept.forEach(g => needed.set(g.ability, (needed.get(g.ability) || 0) + g.count));
    // Under each count, one circle per kind of Aniimo (with "×N" when several are the same): its
    // level inside (a dot for any level), a ring for the personality bonus, and what it's for in
    // the tooltip.
    const dot = (ability, text, bonus, tip) => {
        const a = ABILITY_BY_NAME.get(ability);
        return `<span class="ability-dot small${a && a.dark ? ' dark' : ''}${bonus ? ' bonus' : ''}" style="--ability:${a ? a.color : '#888888'}" title="${tip}" aria-label="${tip}">${text}</span>`;
    };
    const teamDots = g => {
        const where = whereText(g);
        const tip = `${g.count > 1 ? `${g.count}× ` : ''}${g.label}${g.bonus ? ' (+20% speed)' : ''} · ${where}`;
        const times = g.count > 1 ? `<span class="ability-times">×${g.count}</span>` : '';
        return `<span class="ability-kind">${dot(g.ability, g.environment ? '·' : g.level, g.bonus, tip)}${times}</span>`;
    };
    document.getElementById('aniimo-abilities').innerHTML = ABILITIES.map(a => {
        const n = a.name === 'Hauling' ? `${needed.get(a.name) + 1}+` : needed.get(a.name);
        const zero = n === 0;
        const dots = kept
            .filter(g => g.ability === a.name)
            .sort((x, y) => y.level - x.level || Number(y.bonus) - Number(x.bonus))
            .map(teamDots);
        if (a.name === 'Hauling') {
            dots.push(`<span class="ability-kind">${dot('Hauling', '·', false, 'Hauling, any level · carries produce to storage; add more if produce piles up')}</span>`);
        }
        const stack = dots.length ? `<div class="ability-stack">${dots.join('')}</div>` : '';
        return `<div class="ability-col" style="--ability:${a.color}">
            <div class="ability-cell${zero ? ' zero' : ''}" title="${a.name}: ${a.about}">
                <span class="ability-count">${n}</span><span class="ability-name">${a.name}</span>
            </div>${stack}</div>`;
    }).join('');
    container.innerHTML = `
        <div class="table-wrapper">
            <table class="facility-plan-table">
                <thead><tr><th>Aniimo</th><th>How many</th><th>Busy on average</th><th>Where</th></tr></thead>
                <tbody>${rows}${haulingRow}</tbody>
            </table>
        </div>
        ${capNote}
    `;
}

// Splits one environment mode's rows across its individual building units. Unlike the old
// preset-based version, each unit's exact facility-type capacity now comes straight from the
// solver's own geometric packing (`assignment.layouts[i]`; see `FacilityPlacement` in
// models.rs), not an evenly-divided share, since real per-building layouts aren't always
// identical (e.g. one Cooling Unit might host Farmland+Woodland while another hosts only
// Farmland). Still greedily fills each unit's per-facility-type capacity in row order, splitting
// a single row across units when its count exceeds one unit's remaining capacity; the exact
// split is arbitrary (any unit can host any plot of the crops sharing its mode), only the
// per-unit totals (and the diagram's exact positions) are load-bearing.
function splitByEnvironmentUnit(rows, assignmentsForMode) {
    const units = [];
    assignmentsForMode.forEach(a => {
        (a.layouts || []).forEach(layout => {
            const remaining = {};
            layout.forEach(p => {
                remaining[p.facility] = (remaining[p.facility] || 0) + 1;
            });
            units.push({ building: a.building, remaining, rows: [], layout, partner: a.partner || null, zone: a.zone ?? null, pairModes: a.pair_modes || null });
        });
    });

    rows.forEach(step => {
        let remaining = step.facility_count;
        for (const unit of units) {
            if (remaining <= 0) break;
            const available = unit.remaining[step.facility] || 0;
            const take = Math.min(remaining, available);
            if (take <= 0) continue;
            unit.remaining[step.facility] -= take;
            unit.rows.push({ ...step, facility_count: take });
            remaining -= take;
        }
    });

    // A building's geometric layout is capacity, not a production guarantee; a facility type can
    // sit unused in a unit's coverage if there wasn't enough demand to fill every plot the fill
    // loop above offered it. Drawing that unused capacity in the diagram would show the player
    // squares they shouldn't actually place anything in (and that don't match this unit's own
    // table), so trim `layout` down to just the placements this unit's `rows` actually accounted
    // for, per facility type.
    units.forEach(unit => {
        const totalByFacility = {};
        unit.layout.forEach(p => {
            totalByFacility[p.facility] = (totalByFacility[p.facility] || 0) + 1;
        });
        const takenSoFar = {};
        unit.layout = unit.layout.filter(p => {
            const unused = unit.remaining[p.facility] || 0;
            const used = (totalByFacility[p.facility] || 0) - unused;
            takenSoFar[p.facility] = takenSoFar[p.facility] || 0;
            if (takenSoFar[p.facility] < used) {
                takenSoFar[p.facility]++;
                return true;
            }
            return false;
        });
    });

    return units.filter(u => u.rows.length > 0);
}

// Fixed color per environment-gated facility type, used by the layout diagram below; purely
// categorical (not theme-dependent), so it stays distinguishable in both light and dark mode.
const ENVIRONMENT_FACILITY_COLORS = {
    'Farmland': '#8b5e34',
    'Woodland': '#4caf50',
    'Starfall Hammock': '#42a5f5',
    'Tidewhisper Sandcastle': '#26c6da',
    'Floral Windmill': '#ab47bc',
    'Dewy House': '#ef8a80',
};

// Matches the confirmed geometry in src/coverage.rs: every environment building is a 2x2
// footprint, radiating coverage as a square of side 2*radius centered on its own center.
const ENVIRONMENT_BUILDING_SIZE = 2.0;
const ENVIRONMENT_COVERAGE_RADIUS = 4.5;

// Coverage tint for each growing environment, used to shade a building's coverage area.
// Adequate's yellow carries further than the others at the same opacity, so it's laid on lighter.
const ENVIRONMENT_MODE_SHADE = { Adequate: 0.55 };

const ENVIRONMENT_MODE_COLORS = {
    Warm: '#f59e0b',
    Scorching: '#ef4444',
    Cool: '#67e8f9',
    Freeze: '#2563eb',
    Adequate: '#facc15',
};

// Renders one building's layout as an SVG: faint one-tile gridlines, the building, its coverage
// area shaded in the environment's color, and every plot the plan puts in it, nearest the
// building first. `rows` are this building's plan rows; each plot is matched to one of them so
// hovering a plot names its crop, and when one facility type grows more than one crop here (so
// color alone can't tell them apart) each plot shows its crop's number from the legend.
// Positions are the solver's own, in game tiles.
// What each environment building does, drawn on it in the layout diagram, after the game's own
// symbols: a flame for Warm and two for Scorching, a six-armed snowflake for Cool and two for
// Freeze, a sun for Adequate. Paths rather than emoji, which some systems render in their own
// colours.
const ENVIRONMENT_ICON_INK = 'rgba(255, 255, 255, 0.92)';

function flameIcon(cx, cy, size) {
    const s = size;
    return `<path d="M ${cx} ${cy - 0.62 * s}
                     C ${cx + 0.12 * s} ${cy - 0.3 * s} ${cx + 0.42 * s} ${cy - 0.16 * s} ${cx + 0.4 * s} ${cy + 0.12 * s}
                     C ${cx + 0.38 * s} ${cy + 0.42 * s} ${cx + 0.16 * s} ${cy + 0.6 * s} ${cx} ${cy + 0.6 * s}
                     C ${cx - 0.16 * s} ${cy + 0.6 * s} ${cx - 0.4 * s} ${cy + 0.42 * s} ${cx - 0.4 * s} ${cy + 0.1 * s}
                     C ${cx - 0.4 * s} ${cy - 0.1 * s} ${cx - 0.24 * s} ${cy - 0.18 * s} ${cx - 0.18 * s} ${cy - 0.36 * s}
                     C ${cx - 0.1 * s} ${cy - 0.22 * s} ${cx - 0.04 * s} ${cy - 0.3 * s} ${cx} ${cy - 0.62 * s} Z"
                   fill="${ENVIRONMENT_ICON_INK}" />`;
}

function snowflakeIcon(cx, cy, size) {
    const arms = [0, 60, 120].map(angle => {
        const radians = angle * Math.PI / 180;
        const [dx, dy] = [Math.cos(radians) * 0.62 * size, Math.sin(radians) * 0.62 * size];
        return `<line x1="${cx - dx}" y1="${cy - dy}" x2="${cx + dx}" y2="${cy + dy}" />`;
    }).join('');
    return `<g stroke="${ENVIRONMENT_ICON_INK}" stroke-width="${0.17 * size}" stroke-linecap="round">${arms}</g>`;
}

function sunIcon(cx, cy, size) {
    const rays = [0, 45, 90, 135, 180, 225, 270, 315].map(angle => {
        const radians = angle * Math.PI / 180;
        const [dx, dy] = [Math.cos(radians), Math.sin(radians)];
        return `<line x1="${cx + dx * 0.42 * size}" y1="${cy + dy * 0.42 * size}"
                      x2="${cx + dx * 0.64 * size}" y2="${cy + dy * 0.64 * size}" />`;
    }).join('');
    return `<circle cx="${cx}" cy="${cy}" r="${0.28 * size}" fill="${ENVIRONMENT_ICON_INK}" />
            <g stroke="${ENVIRONMENT_ICON_INK}" stroke-width="${0.15 * size}" stroke-linecap="round">${rays}</g>`;
}

function environmentBuildingIcon(building, mode, cx, cy) {
    // The stronger of a building's two modes shows its symbol twice, as the game does: the main
    // one low and left, a smaller one off its top right, both clear of the building's edge.
    const twice = draw => `${draw(cx - 0.12, cy + 0.14, 0.92)}${draw(cx + 0.38, cy - 0.32, 0.56)}`;
    if (building === 'Heat Furnace') {
        return mode === 'Scorching' ? twice(flameIcon) : flameIcon(cx, cy, 1.0);
    }
    if (building === 'Cooling Unit') {
        return mode === 'Freeze' ? twice(snowflakeIcon) : snowflakeIcon(cx, cy, 1.0);
    }
    if (building === 'Sunlamp') {
        return sunIcon(cx, cy, 1.0);
    }
    return '';
}

function renderEnvironmentDiagram(layout, mode, building, rows = [], unit = null, zones = null) {
    if (!layout || layout.length === 0) return '';
    const margin = 5;
    const half = ENVIRONMENT_COVERAGE_RADIUS + margin;
    const buildingCenter = ENVIRONMENT_BUILDING_SIZE / 2;
    // Centered on the building's own center (it sits at (0,0)-(size,size)), not world origin.
    const viewMin = buildingCenter - half;
    const viewSize = half * 2;
    const coverageMin = buildingCenter - ENVIRONMENT_COVERAGE_RADIUS;
    const coverageSize = ENVIRONMENT_COVERAGE_RADIUS * 2;
    const tint = ENVIRONMENT_MODE_COLORS[mode] || '#9aa0a8';
    // Two buildings placed to overlap: the plots shown are one of the three zones they make, in
    // a frame with this building at the origin and its partner dx tiles along and dy tiles up.
    const dx = unit && unit.partner ? unit.partner[1] : 0;
    const dy = unit && unit.partner ? unit.partner[2] : 0;
    const zone = unit && unit.partner ? unit.zone : null;
    const viewWidth = viewSize + dx;
    const viewHeight = viewSize + dy;
    // Each building covers its own 9x9 square; this zone is the part of them that gives this
    // temperature: only the first's, only the second's, or where the two meet.
    const modes = unit && unit.pairModes ? unit.pairModes : null;
    const tintOf = m => ENVIRONMENT_MODE_COLORS[m] || '#9aa0a8';
    const shadeOf = (m, opacity) => (opacity * (ENVIRONMENT_MODE_SHADE[m] ?? 1)).toFixed(3);
    // Where the two coverage squares cross. It is always a rectangle; each building's own zone is
    // its square with that rectangle taken out, which is an L unless the buildings line up.
    const shared = { x: coverageMin + dx, y: coverageMin + dy, w: coverageSize - dx, h: coverageSize - dy };
    const box = (x, y, w, h) => `M${x} ${y}h${w}v${h}h${-w}Z`;
    // A zone as one path: the shared rectangle on its own, or a square with it cut out (two
    // subpaths, which the even-odd rule reads as the difference).
    const zonePath = z => {
        if (z === 1) return box(shared.x, shared.y, shared.w, shared.h);
        const own = z === 2 ? box(coverageMin + dx, coverageMin + dy, coverageSize, coverageSize)
            : box(coverageMin, coverageMin, coverageSize, coverageSize);
        return `${own}${box(shared.x, shared.y, shared.w, shared.h)}`;
    };

    // Gridlines like the game's: stronger on whole tiles, very faint on the quarter tiles
    // facilities snap to.
    const gridLines = [];
    for (let t = Math.ceil(viewMin * 4) / 4; t <= viewMin + Math.max(viewWidth, viewHeight); t += 0.25) {
        // Every other tile line is a major one, in step with the 2x2 buildings.
        const cls = !Number.isInteger(t) ? 'quarter' : t % 2 === 0 ? 'tile major' : 'tile';
        if (t <= viewMin + viewWidth) {
            gridLines.push(`<line class="${cls}" x1="${t}" y1="${viewMin}" x2="${t}" y2="${viewMin + viewHeight}" />`);
        }
        if (t <= viewMin + viewHeight) {
            gridLines.push(`<line class="${cls}" x1="${viewMin}" y1="${t}" x2="${viewMin + viewWidth}" y2="${t}" />`);
        }
    }

    // Plots nearest the building first, each matched to a plan row of its facility type. On a
    // shared map every zone matches its own rows to its own plots.
    const distance = p => Math.hypot(p.x + p.size / 2 - buildingCenter, p.y + p.size / 2 - buildingCenter);
    const matchCrops = (plots, plotRows) => {
        const queue = {};
        plotRows.forEach(r => {
            if (!r.item_name) return;
            (queue[r.facility] = queue[r.facility] || []).push({ item: r.item_name, left: r.facility_count });
        });
        return [...plots].sort((a, b) => distance(a) - distance(b)).map(p => {
            const q = queue[p.facility];
            while (q && q.length && q[0].left <= 0) q.shift();
            if (!q || !q.length) return { ...p, crop: null };
            q[0].left--;
            return { ...p, crop: q[0].item };
        });
    };
    const assigned = zones ? zones.flatMap(z => matchCrops(z.layout, z.rows)) : matchCrops(layout, rows);
    const crops = [...new Set(assigned.map(p => `${p.facility}|${p.crop}`))];
    const cropsPerFacility = {};
    crops.forEach(key => {
        const facility = key.split('|')[0];
        cropsPerFacility[facility] = (cropsPerFacility[facility] || 0) + 1;
    });
    const numbered = Object.values(cropsPerFacility).some(n => n > 1);
    const numberOf = key => crops.indexOf(key) + 1;

    // A small inset keeps edge-touching plots visibly separate; purely cosmetic.
    const inset = 0.08;
    const rects = assigned.map(p => {
        const color = ENVIRONMENT_FACILITY_COLORS[p.facility] || '#888888';
        const size = p.size - inset * 2;
        const label = p.crop ? `${p.facility}: ${prettyItem(p.crop)}` : p.facility;
        const initials = numbered && p.crop
            ? `<text x="${p.x + p.size / 2}" y="${p.y + p.size / 2}" font-size="${Math.min(0.9, p.size * 0.4)}">${numberOf(`${p.facility}|${p.crop}`)}</text>`
            : '';
        return `<g class="env-plot"><title>${label}</title>
            <rect x="${p.x + inset}" y="${p.y + inset}" width="${size}" height="${size}" rx="0.25" fill="${color}" fill-opacity="0.85" stroke="${color}" stroke-width="0.06" />${initials}</g>`;
    }).join('');

    const counts = {};
    assigned.forEach(p => {
        const key = `${p.facility}|${p.crop}`;
        counts[key] = (counts[key] || 0) + 1;
    });
    // What the colours mean: the coverage, then each crop with how many plots it gets here.
    const swatch = (color, label) => `
        <span class="env-legend-item">
            <span class="env-legend-swatch coverage" style="background:${color}33;border-color:${color}"></span>${label}
        </span>`;
    // On a shared map the heading already names both buildings and what they're set to; the
    // legend only has to say what the middle is and which crop is which.
    const coverageLegend = modes
        ? (zones
            ? []
            : [
                swatch(tintOf(modes[0]), `${building}: ${modes[0]}`),
                swatch(tintOf(modes[1]), `${unit.partner[0]}: ${modes[1]}`),
                swatch(tint, zone === 1 ? `${mode} where both reach` : `${mode}, this plan's plots`),
            ])
        : [swatch(tint, `${mode} coverage`)];
    const legend = coverageLegend.concat(Object.entries(counts).map(([key, n]) => {
        const [facility, crop] = key.split('|');
        // The number a plot carries in the diagram reads as part of the facility's name, as the
        // plots themselves do: "Farmland 1: Sugarcane".
        const named = `${facility}${numbered ? ` <b>${numberOf(key)}</b>` : ''}`;
        const name = crop && crop !== 'null' ? `${named}: ${prettyItem(crop)}` : named;
        return `
        <span class="env-legend-item">
            <span class="env-legend-swatch" style="background:${ENVIRONMENT_FACILITY_COLORS[facility] || '#888888'}"></span>${name} ×${n}
        </span>`;
    })).join('');

    return `
        <div class="env-diagram">
            <svg viewBox="${viewMin} ${viewMin} ${viewWidth} ${viewHeight}" role="img" aria-label="${building} layout, ${mode} coverage">
                <g class="env-grid">${gridLines.join('')}</g>
                <rect x="${coverageMin}" y="${coverageMin}" width="${coverageSize}" height="${coverageSize}"
                      fill="${modes ? tintOf(modes[0]) : tint}" fill-opacity="${shadeOf(modes ? modes[0] : mode, 0.12)}" />
                ${modes ? `<rect x="${coverageMin + dx}" y="${coverageMin + dy}" width="${coverageSize}" height="${coverageSize}"
                      fill="${tintOf(modes[1])}" fill-opacity="${shadeOf(modes[1], 0.12)}" />` : ''}
                ${rects}
                ${(zones || [{ mode, zone }]).map(z => {
                    const path = z.zone === null || z.zone === undefined
                        ? box(coverageMin, coverageMin, coverageSize, coverageSize)
                        : zonePath(z.zone);
                    const zoneTint = tintOf(z.mode);
                    return `<path class="env-coverage-top" d="${path}" fill-rule="evenodd"
                      fill="${zoneTint}" fill-opacity="${shadeOf(z.mode, 0.3)}" stroke="${zoneTint}" stroke-opacity="0.9" stroke-dasharray="0.35,0.25" stroke-width="0.1" />`;
                }).join('')}
                ${modes ? `<rect x="${coverageMin}" y="${coverageMin}" width="${coverageSize}" height="${coverageSize}" fill="none"
                      stroke="${tintOf(modes[0])}" stroke-opacity="0.55" stroke-width="0.07" />
                    <rect x="${coverageMin + dx}" y="${coverageMin + dy}" width="${coverageSize}" height="${coverageSize}" fill="none"
                      stroke="${tintOf(modes[1])}" stroke-opacity="0.55" stroke-width="0.07" />` : ''}
                <g class="env-building"><title>${building} (${modes ? modes[0] : mode})</title>
                    <rect x="0.05" y="0.05" width="${ENVIRONMENT_BUILDING_SIZE - 0.1}" height="${ENVIRONMENT_BUILDING_SIZE - 0.1}" rx="0.3"
                          fill="${modes ? tintOf(modes[0]) : tint}" stroke="currentColor" stroke-opacity="0.6" stroke-width="0.08" />
                    ${environmentBuildingIcon(building, modes ? modes[0] : mode, buildingCenter, buildingCenter)}
                </g>
                ${unit && unit.partner ? `<g class="env-building"><title>${unit.partner[0]} (${modes ? modes[1] : mode})</title>
                    <rect x="${dx + 0.05}" y="${dy + 0.05}" width="${ENVIRONMENT_BUILDING_SIZE - 0.1}" height="${ENVIRONMENT_BUILDING_SIZE - 0.1}" rx="0.3"
                          fill="${modes ? tintOf(modes[1]) : tint}" stroke="currentColor" stroke-opacity="0.6" stroke-width="0.08" />
                    ${environmentBuildingIcon(unit.partner[0], modes ? modes[1] : mode, dx + buildingCenter, dy + buildingCenter)}
                </g>` : ''}
            </svg>
            <div class="env-legend">${legend}</div>
        </div>
    `;
}

// Renders `plan.coin_items` (one row per facility+product; see `PlanStep` in models.rs). Rows
// for a crop that needs a growing environment (Cool/Warm/Freeze/Scorching/Adequate) are pulled
// out into their own "Environment: X" group first; regardless of whether they're grown on
// Farmland or Woodland; so it's obvious at a glance which facilities share the same environment
// building, instead of that connection being spelled out in each row's own text. When a mode
// needs more than one building unit, that group splits into one table per unit (see
// `splitByEnvironmentUnit`) so it's clear which crops go in which physical building. Everything
// else falls back to the original per-facility-category grouping (FACILITY_CATEGORIES).
function renderFacilityPlan(plan) {
    const container = document.getElementById('facility-plan-container');
    const steps = plan.coin_items || [];

    if (steps.length === 0) {
        container.innerHTML = '<p class="hint">Nothing profitable to produce with the current facilities.</p>';
        return;
    }

    const envGroups = new Map();
    const ungatedSteps = [];
    steps.forEach(step => {
        if (step.environment) {
            if (!envGroups.has(step.environment)) envGroups.set(step.environment, []);
            envGroups.get(step.environment).push(step);
        } else {
            ungatedSteps.push(step);
        }
    });

    const assignments = plan.environment_assignments || [];
    // Split every mode's crops across the buildings covering them, then show one map per
    // building: two that overlap share a single map, since they're one place on the homeland.
    const byMode = new Map();
    ENVIRONMENT_MODE_ORDER.filter(mode => envGroups.has(mode)).forEach(mode => {
        byMode.set(mode, splitByEnvironmentUnit(envGroups.get(mode), assignments.filter(a => a.mode === mode)));
    });
    const pairs = new Map();
    const singles = [];
    byMode.forEach((units, mode) => {
        units.forEach(unit => {
            if (!unit.partner) {
                singles.push({ mode, unit });
                return;
            }
            const key = `${unit.building}|${unit.partner[0]}|${unit.partner[1]},${unit.partner[2]}`;
            if (!pairs.has(key)) pairs.set(key, { unit, zones: [] });
            pairs.get(key).zones.push({ mode, layout: unit.layout, rows: unit.rows, zone: unit.zone });
        });
    });

    const pairSections = [...pairs.values()].map(({ unit, zones }) => {
        zones.sort((a, b) => a.zone - b.zone);
        const modes = unit.pairModes || [unit.mode, unit.mode];
        const middle = zones.find(z => z.zone === 1)?.mode;
        return `
            <div class="facility-category">
                <h4 class="facility-category-title">${unit.building} ${modeTag(modes[0])}<span class="env-head-sep">|</span>${unit.partner[0]} ${modeTag(modes[1])}${middle ? `<span class="env-head-sep">|</span>Overlap ${modeTag(middle)}` : ''}</h4>
                <p class="hint small">${gapText(unit.partner[1], unit.partner[2])}</p>
                <div class="env-unit">
                    ${renderEnvironmentDiagram(zones.flatMap(z => z.layout), zones[0].mode, unit.building, zones.flatMap(z => z.rows), unit, zones)}
                    <div class="env-unit-table">${facilityPlanTableOf(zones.map(z => ({ label: modeTag(z.mode), rows: z.rows })))}</div>
                </div>
            </div>
        `;
    }).join('');

    const modeSections = ENVIRONMENT_MODE_ORDER.map(mode => {
        const units = singles.filter(s => s.mode === mode).map(s => s.unit);
        if (units.length === 0) {
            // Crops wanting this mode that no building's map accounted for, if any: the units
            // hold copies of each row, so compare by how many plots each one placed.
            const placed = {};
            (byMode.get(mode) || []).forEach(u => u.rows.forEach(r => {
                placed[`${r.facility}|${r.item_name}`] = (placed[`${r.facility}|${r.item_name}`] || 0) + r.facility_count;
            }));
            const orphans = (envGroups.get(mode) || [])
                .map(step => ({ ...step, facility_count: step.facility_count - (placed[`${step.facility}|${step.item_name}`] || 0) }))
                .filter(step => step.facility_count > 0);
            return orphans.length === 0 ? '' : `
                <div class="facility-category">
                    <h4 class="facility-category-title">${modeTag(mode)}</h4>
                    ${facilityPlanTable(orphans)}
                </div>`;
        }
        return `
            <div class="facility-category">
                <h4 class="facility-category-title">${units[0].building} ${modeTag(mode)}</h4>
                ${units.map((unit, i) => `
                    ${units.length > 1 ? `<p class="hint small">${unit.building} ${i + 1}</p>` : ''}
                    <div class="env-unit">
                        ${renderEnvironmentDiagram(unit.layout, mode, unit.building, unit.rows, unit)}
                        <div class="env-unit-table">${facilityPlanTable(unit.rows)}</div>
                    </div>
                `).join('')}
            </div>
        `;
    }).join('');
    const environmentSections = pairSections + modeSections;

    const byCategory = new Map(FACILITY_CATEGORIES.map(c => [c, []]));
    ungatedSteps.forEach(step => {
        const category = FACILITY_CATEGORY_BY_NAME.get(step.facility) || 'Materials Processing';
        byCategory.get(category).push(step);
    });

    const categorySections = FACILITY_CATEGORIES.map(category => {
        const categorySteps = byCategory.get(category);
        if (categorySteps.length === 0) return '';
        return `
            <div class="facility-category">
                <h4 class="facility-category-title">${category}</h4>
                ${facilityPlanTable(categorySteps)}
            </div>
        `;
    }).join('');

    container.innerHTML = environmentSections + categorySections;
}

// Re-renders "Your Rate" from `lastPlan` at whichever unit is currently selected in the
// `#rate-unit` dropdown; called after a fresh plan and again whenever the user switches units, so
// switching units never needs a facility-allocation re-solve. `pickUnit` is for a fresh plan
// only: it chooses the unit the plan reads best at. Switching units must never do that, or the
// choice would be undone the moment it's made.
function updateRateDisplay(pickUnit = false) {
    if (!lastPlan || !lastPlan.success) return;
    const select = document.getElementById('rate-unit');
    const rows = planContext && !planContext.levelUp ? priorityRows(lastPlan) : null;
    // Step the unit up until the smallest rate reads at least 1 (Aniipods per hour, Rough Lumber
    // per hour, not 0.01 per second); a plain coin rate only needs to read above zero.
    const costRates = (lastPlan.level_up?.requirements || []).map(r => r.per_second);
    const rates = (rows ? rows.map(r => r.perSecond) : costRates).filter(r => r > 1e-9);
    const smallest = rates.length ? Math.min(...rates) : lastPlan.rate_per_second;
    const least = rates.length ? 1 : 0.05;
    while (pickUnit && smallest * RATE_UNIT_SECONDS[select.value].multiplier < least) {
        const next = { second: 'minute', minute: 'hour', hour: 'day' }[select.value];
        if (!next) break;
        select.value = next;
    }
    const { multiplier, suffix } = RATE_UNIT_SECONDS[select.value] || RATE_UNIT_SECONDS.second;
    const rateLine = document.getElementById('rate-line');
    const table = document.getElementById('priority-rates');
    if (!rows) {
        if (select.closest('#priority-rates')) rateLine.appendChild(select);
        const label = CURRENCY_LABELS[lastPlan.currency] || lastPlan.currency;
        document.getElementById('plan-rate').textContent = `${formatNumber(lastPlan.rate_per_second * multiplier)} ${label}${suffix}`;
        document.getElementById('rate-label').textContent = 'Your rate';
        rateLine.style.display = '';
        table.innerHTML = '';
        return;
    }
    // Priorities: one row each, in the player's order, at the selected unit.
    const amount = formatRate;
    const body = rows.map(r => {
        const why = r.perSecond <= 1e-9 && r.missing ? `<span class="hint small">${r.missing}</span>` : '';
        // Aniimo EXP names the Growth items making it; an Aniipod row is already named by tier.
        const made = r.target === 'aniimo_exp'
            ? r.items.filter(([, n]) => n > 1e-9).map(([item, n]) => `${amount(n * multiplier)} ${prettyItem(item)}`).join(', ')
            : '';
        return `<tr${r.rank === 1 ? ' class="top"' : ''}>
            <td>${r.rank ?? ''}</td>
            <td>${r.label}</td>
            <td>${why ? 'none' : amount(r.perSecond * multiplier)}</td>
            <td>${why || made}</td>
        </tr>`;
    }).join('');
    table.innerHTML = `
        <table class="level-up-lines rate-table">
            <thead><tr><th>#</th><th>Priority</th><th id="priority-rate-head"></th><th></th></tr></thead>
            <tbody>${body}</tbody>
        </table>`;
    document.getElementById('priority-rate-head').appendChild(select);
    document.getElementById('rate-label').textContent = 'Your rates';
    rateLine.style.display = 'none';
}

// A Priorities plan's rate rows, in the player's order: each priority, then coins from what's
// left if coins wasn't one of them. A priority the homeland can't make yet says why.
function priorityRows(plan) {
    const missing = {
        aniimo_exp: planContext?.hasPolisher ? null : 'No Dance Pad Polisher yet',
        aniipods: planContext?.aniipod ? null : 'No Aniipod Maker yet',
    };
    const rows = (plan.priorities || []).map((p, i) => ({
        rank: i + 1,
        target: p.target,
        label: priorityLabel(p.target, planContext?.aniipod),
        perSecond: p.per_second,
        items: p.items || [],
        missing: missing[p.target] || null,
    }));
    if (!rows.some(r => r.target === 'coins')) {
        rows.push({ rank: null, target: 'coins', label: rows.length ? "Coins, from what's left" : 'Coins', perSecond: plan.rate_per_second, items: [], missing: null });
    }
    return rows;
}

// Re-renders every rate-unit-dependent display ("Your Rate" and the Product Breakdown table's
// Profit column) from the already-computed `lastPlan`/`lastGoalResult`; the `#rate-unit` change
// listener target, so switching units never needs a re-solve.
function updateRateUnitDisplays() {
    updateRateDisplay();
    if (lastPlan && lastPlan.success) {
        renderSeedTable(lastPlan);
        renderLevelUp(lastPlan);
    }
    if (lastGoalResult) {
        renderProductBreakdown(lastGoalResult);
    }
}

// Render a successfully computed plan: rate summary + facility plan table. Goal-independent,
// called once per Calculate click (or facility/currency/module change), not on every goal
// keystroke.
function displayPlan(plan, scroll = true) {
    const resultsSection = document.getElementById('results-section');
    const errorEl = document.getElementById('error-message');
    const resultsContent = document.getElementById('results-content');
    const goalSection = document.getElementById('goal-section');

    resultsSection.style.display = 'block';

    if (!plan.success) {
        goalSection.style.display = 'none';
        showError(plan.error || 'An unknown error occurred.');
        return;
    }

    errorEl.style.display = 'none';
    resultsContent.style.display = 'block';
    // A level-up plan's own card says how long it takes; the goal is for coin plans.
    goalSection.style.display = plan.level_up ? 'none' : 'block';

    updateRateDisplay(!rateUnitChosen);
    renderGoalTargets(plan);

    // Said only when the plan might not be the best: the solver ran out of time, or the backup
    // planner made it.
    const explored = document.getElementById('plan-explored-hint');
    explored.style.display = plan.proven_optimal === true ? 'none' : '';
    if (plan.proven_optimal === true) {
        explored.textContent = '';
    } else if (plan.proven_optimal === false && plan.upper_bound > 0) {
        const gap = Math.max(0, (plan.upper_bound - plan.rate_per_second) / plan.upper_bound * 100);
        explored.textContent = `Best plan found in the time allowed; the best possible is at most ${gap.toFixed(1)}% higher.`;
    } else {
        const reason = plan.fallback_reason ? ` (${plan.fallback_reason})` : '';
        explored.textContent = `The exact planner couldn't run${reason}, so this plan comes from the backup planner and may not be the very best. Reloading the page usually fixes this.`;
    }

    const unverifiedEl = document.getElementById('plan-unverified');
    const unverified = plan.unverified || [];
    unverifiedRowKeys = new Set(unverified.map(u => `${u.facility}|${u.item_name}`));
    if (unverified.length) {
        unverifiedEl.textContent = `${unverified.length} recipe${unverified.length === 1 ? '' : 's'} in this plan ${unverified.length === 1 ? "hasn't" : "haven't"} been checked in game yet (tagged below). If any of those numbers are off, so is this plan.`;
        unverifiedEl.style.display = 'block';
    } else {
        unverifiedEl.style.display = 'none';
    }

    const skippedEl = document.getElementById('plan-skipped');
    const skipped = planContext?.skipped || [];
    skippedEl.style.display = skipped.length ? 'block' : 'none';
    skippedEl.textContent = skipped.length ? `Skipping ${skipped.map(prettyItem).join(', ')}.` : '';

    renderSeedTable(plan);
    renderLevelUp(plan);
    renderProfitBreakdown(plan);
    renderFacilityPlan(plan);
    renderAniimoSummary(plan);

    if (scroll) resultsSection.scrollIntoView({ behavior: 'smooth' });
}

// Render a time-to-goal result: Total Time / Amount Produced summary + Product Breakdown. Called
// live on every goal-field keystroke once a plan exists; cheap, no facility-allocation re-solve.
function displayGoal(goalResult) {
    if (!goalResult.success) {
        lastGoalResult = null;
        document.getElementById('total-time').textContent = '-';
        document.getElementById('amount-produced').textContent = '-';
        document.getElementById('product-breakdown-section').style.display = 'none';
        document.getElementById('seeds-needed-section').style.display = 'none';
        console.warn('Goal calculation failed:', goalResult.error);
        return;
    }

    lastGoalResult = goalResult;
    document.getElementById('total-time').textContent = goalResult.total_time_seconds > 0 ? formatDuration(goalResult.total_time_seconds) : '0m';
    document.getElementById('amount-produced').textContent = formatNumber(goalResult.amount_produced);

    renderProductBreakdown(goalResult);
    renderSeedsNeeded(goalResult);
}

// Solve for the best achievable plan (facilities + currency + modules); the heavier computation,
// triggered explicitly by the Calculate button or Enter in a facility/module field.
async function runFindPlan() {
    if (!wasmReady) {
        showError('Optimizer not ready. Please wait...');
        return;
    }

    const btn = document.getElementById('optimize-btn');
    const btnText = btn.querySelector('.btn-text');
    const btnLoading = btn.querySelector('.btn-loading');
    const progressBar = document.getElementById('progress-bar-container');
    const progressFill = document.getElementById('progress-bar-fill');
    const progressCaption = document.getElementById('progress-bar-caption');

    btn.disabled = true;
    btnText.style.display = 'none';
    btnLoading.style.display = 'inline';
    progressBar.style.display = 'block';
    progressCaption.style.display = 'block';
    progressFill.style.width = '';
    progressFill.classList.add('indeterminate');
    progressCaption.textContent = 'Finding the best plan...';

    const runId = ++planRunId;
    plansBySetup = {};
    if (pendingWorkerRequests.size > 0) restartWorker();
    try {
        const input = getPlanInputValues();
        planContext = {
            levelUp: isLevelUpStrategy(),
            target: levelUpTarget(),
            unavailable: levelUpUnavailable(),
            ready: !!(input.level_up && input.level_up.cost.every(([name, need]) => stockAmount(name) >= need)),
            aniipod: wantsAniipods() ? bestAniipod() : null,
            hasPolisher: (input.facilities['Dance Pad Polisher'] || []).some(t => t.count > 0),
            // Only what the player skipped; locked special recipes are the default, not news.
            skipped: [...skippedRecipes].sort((a, b) => prettyItem(a).localeCompare(prettyItem(b))),
        };

        // Runs in the worker (see worker.js); the main thread stays free to paint the progress
        // bar above for however long this takes, instead of freezing. `onTrialProgress` receives
        // the solver's own real, running trial-solve count after every trial solve; converted to
        // a fill percentage by `trialCountToPercent` below.
        // Only the backup planner reports progress (see worker.js); the exact planner is quick.
        const bestJson = await callWorker('find_plan', JSON.stringify({ ...input, aniimo: 'best' }), (count) => {
            progressFill.classList.remove('indeterminate');
            progressFill.style.width = `${trialCountToPercent(count)}%`;
            progressCaption.textContent = `Backup planner, trial ${count}...`;
        });
        progressFill.style.width = '100%';
        if (runId !== planRunId) return;
        plansBySetup.best = JSON.parse(bestJson);
        showSelectedPlan(true);

        // The Minimum setup solves after Best is already on screen; switching to it before it's
        // done shows a short "still working" note until it arrives.
        callWorker('find_plan', JSON.stringify({ ...input, aniimo: 'minimum' }))
            .then(json => {
                if (runId !== planRunId) return;
                plansBySetup.minimum = JSON.parse(json);
                if (selectedAniimoSetup() === 'minimum') showSelectedPlan(false);
            })
            .catch(error => {
                if (runId === planRunId) console.error('Minimum Aniimo plan failed:', error);
            });
    } catch (error) {
        console.error('Plan calculation error:', error);
        lastPlan = null;
        showError(`Plan calculation failed: ${error.message}`);
    } finally {
        btn.disabled = false;
        btnText.style.display = 'inline';
        btnLoading.style.display = 'none';
        progressBar.style.display = 'none';
        progressCaption.style.display = 'none';
        progressFill.classList.remove('indeterminate');
    }
}

// Compute time-to-goal against the already-computed `lastPlan`; cheap, safe to call on every
// keystroke of the goal-amount fields. No-op until a plan exists.
async function runTimeToGoal() {
    if (!lastPlan || !lastPlan.success || planContext?.levelUp) return;
    const rows = priorityRows(lastPlan);
    const chosen = rows.find(r => r.target === document.getElementById('goal-target').value) || rows[0];
    const name = goalName(chosen);
    document.getElementById('target-amount-label').textContent = `Target ${name}`;
    document.getElementById('current-amount-label').textContent = `Current ${name}`;
    document.getElementById('amount-produced-label').textContent = `${name} produced`;

    const target = floatOrDefault(document.getElementById('target-amount').value, 0);
    const current = floatOrDefault(document.getElementById('current-amount').value, 0);

    if (chosen.target === 'coins') {
        // Coins account for each product's start-up delay and list what's sold along the way.
        try {
            const resultJson = await callWorker('time_to_reach', JSON.stringify({ plan: lastPlan, target, current }));
            const result = JSON.parse(resultJson);
            displayGoal(result);
            renderGoalAlso(rows, chosen, result.success ? result.total_time_seconds : null);
        } catch (error) {
            console.error('Goal calculation error:', error);
        }
        return;
    }
    // Anything else comes in at its steady rate; the breakdown covers that long.
    const needed = Math.max(0, target - current);
    const seconds = needed <= 0 ? 0 : chosen.perSecond > 1e-12 ? needed / chosen.perSecond : null;
    if (seconds === null) {
        lastGoalResult = null;
        document.getElementById('total-time').textContent = chosen.missing || 'Not made by this plan';
        document.getElementById('amount-produced').textContent = '-';
        document.getElementById('product-breakdown-section').style.display = 'none';
        document.getElementById('seeds-needed-section').style.display = 'none';
        renderGoalAlso(rows, chosen, null);
        return;
    }
    try {
        const resultJson = await callWorker('time_to_reach', JSON.stringify({ plan: lastPlan, seconds }));
        displayGoal(JSON.parse(resultJson));
        document.getElementById('amount-produced').textContent = formatNumber(Math.round(needed));
        renderGoalAlso(rows, chosen, seconds);
    } catch (error) {
        console.error('Goal calculation error:', error);
    }
}

// What else the plan makes by the time the goal is met, e.g. "4.6M coins, 39 Aniipod Mega".
function renderGoalAlso(rows, chosen, seconds) {
    const el = document.getElementById('goal-also');
    const also = seconds > 0
        ? rows.filter(r => r !== chosen && r.perSecond > 1e-12)
            .map(r => `${formatNumber(Math.floor(r.perSecond * seconds))} ${r.target === 'coins' ? 'coins' : goalName(r)}`)
        : [];
    el.style.display = also.length ? 'block' : 'none';
    el.innerHTML = also.length ? `<span>By then you'll also have:</span> <strong>${also.join(', ')}</strong>` : '';
}

// --- Facility recipe reference modal ----------------------------------------------------
// A static reference table of every recipe in the game data, grouped by facility. Unlike the
// facility input cards, this isn't tied to owned facility counts or levels; it just lists what's
// possible to unlock. Recipe data comes from `get_all_items()` (see wasm.rs), which dumps every
// `ProductionItem` unfiltered.

const RECIPE_MODULE_LABELS = {
    ecological_module: 'Ecological Module',
    kitchen_module: 'Kitchen Module',
    resource_detector: 'Resource Detector',
    crafting_module: 'Crafting Module',
};

// Cached after the first render, since the underlying data never changes for a given wasm build.
let recipesRendered = false;

// Mirrors the Rust `format_time` helper in wasm.rs (hours/minutes/seconds, dropping leading
// zero units) so times read the same way here as they would in-game.
function formatRecipeTime(seconds) {
    const total = Math.round(seconds);
    const hours = Math.floor(total / 3600);
    const minutes = Math.floor((total % 3600) / 60);
    const secs = total % 60;
    if (hours > 0) return `${hours}h ${minutes}m ${secs}s`;
    if (minutes > 0) return `${minutes}m ${secs}s`;
    return `${secs}s`;
}

function formatRecipeInputs(recipe) {
    if (recipe.raw_materials && recipe.raw_materials.length > 0) {
        const amounts = recipe.required_amount || [];
        return recipe.raw_materials
            .map((mat, i) => `${amounts[i] ?? '?'}× ${prettyItem(mat)}`)
            .join(', ');
    }
    if (recipe.cost && recipe.cost > 0) {
        return `Plant cost: ${recipe.cost}`;
    }
    return '-';
}

function formatRecipeYield(recipe) {
    let text = `${recipe.yield_amount}`;
    if (recipe.byproduct) {
        const [name, amount] = recipe.byproduct;
        text += ` <span class="hint small">(+${amount} ${name})</span>`;
    }
    return text;
}

// "Fire Lv.2+, best Lv.3 Practical": the minimum ability level a recipe accepts, then the best
// Aniimo for it. Crops and trees list the ability of each job (sowing, reaping and so on).
function formatRecipeAniimo(recipe, facility) {
    if (!recipe.aniimo) {
        const jobs = [];
        (recipe.jobs || []).forEach(job => {
            const last = jobs[jobs.length - 1];
            // Watering is listed once per time it happens; show it as one job done twice.
            if (last && last.job[0] === job[0]) last.times += 1;
            else jobs.push({ job, times: 1 });
        });
        if (jobs.length === 0) return '-';
        return `<span class="job-list">${jobs.map(({ job: [step, ability, level], times }) =>
            `<span class="job"><span class="job-step">${step}${times > 1 ? ` &times;${times}` : ''}</span> ${abilityTag(ability)}${level > 1 ? ` Lv.${level}+` : ''}</span>`).join('')}</span>`;
    }
    const [ability, minLevel] = recipe.aniimo;
    const best = `best Lv.3${facility.personality ? ' ' + facility.personality : ''}`;
    return `<span>${abilityTag(ability)} Lv.${minLevel}+<span class="recipe-best">${best}</span></span>`;
}

// "44 coins", or what a level-up material is for.
function formatRecipeSell(recipe) {
    if (recipe.sell_currency === 'none') return '<span class="hint small">RV level-ups</span>';
    return `${formatNumber(recipe.sell_value)} ${recipe.sell_value === 1 ? 'coin' : 'coins'}`;
}

function formatRecipeModule(recipe) {
    if (!recipe.module_requirement) return '-';
    const [name, level] = recipe.module_requirement;
    const label = RECIPE_MODULE_LABELS[name] || name;
    return `${label} Lv.${level}`;
}

// Renders one table per facility (grouped into category sections, same grouping/order as the
// facility input cards), each listing every recipe available at that facility sorted by required
// level then name.
function renderRecipeTables(recipes) {
    const container = document.getElementById('facilities-modal-container');

    const byFacility = new Map();
    recipes.forEach(r => {
        if (!byFacility.has(r.facility)) byFacility.set(r.facility, []);
        byFacility.get(r.facility).push(r);
    });
    byFacility.forEach(list => {
        list.sort((a, b) => a.facility_level - b.facility_level || a.name.localeCompare(b.name));
    });

    container.innerHTML = FACILITY_CATEGORIES.map(category => {
        const facilitiesInCategory = FACILITIES.filter(f => f.category === category && byFacility.has(f.name));
        if (facilitiesInCategory.length === 0) return '';

        const tables = facilitiesInCategory.map(f => {
            // `data-label` names each cell when rows stack on phones; empty cells are left out there.
            const cell = (label, value) => `<td data-label="${label}"${value === '-' ? ' class="empty"' : ''}>${value}</td>`;
            const rows = byFacility.get(f.name).map(r => `
                <tr${r.verified === false ? ' class="unverified"' : ''}>
                    <td class="recipe-name">${prettyItem(r.name)}${SPECIAL_NAMES.has(r.name) ? ' <span class="tag special" title="Takes a rare currency to unlock">special</span>' : ''}${r.verified === false ? ' <span class="info-icon" data-tooltip="Not yet checked in game.">?</span>' : ''}</td>
                    ${cell('Level', r.facility_level)}
                    ${cell('Inputs', formatRecipeInputs(r))}
                    ${cell('Yield', formatRecipeYield(r))}
                    ${cell('Time', r.workload ? `${r.workload} workload` : formatRecipeTime(r.production_time))}
                    ${cell('Sell', formatRecipeSell(r))}
                    ${cell('Module', formatRecipeModule(r))}
                    ${cell('Aniimo', formatRecipeAniimo(r, f))}
                </tr>
            `).join('');

            return `
                <div class="facility-recipe-table">
                    <h4>${f.name}</h4>
                    <div class="table-wrapper">
                        <table class="recipe-table">
                            <thead>
                                <tr>
                                    <th>Item</th>
                                    <th>Level</th>
                                    <th>Inputs</th>
                                    <th>Yield</th>
                                    <th>Time <span class="info-icon" data-tooltip="Grow time for crops and trees, before watering takes an eighth off it twice. Everything else lists workload: at 100% Efficiency a processor gets through one workload a second, a gathering facility 1.25 on a level-2 recipe and 1.5 on a level-3 one. An Aniimo at the level a recipe needs works at 100%; higher levels are faster (at a processor, 300% one level above, then +100% per level; at gathering facilities, +50% per level on a level-1 recipe and +40% on harder ones).">?</span></th>
                                    <th>Sell</th>
                                    <th>Module</th>
                                    <th>Aniimo <span class="info-icon" data-tooltip="The lowest ability level that can make this, and the best Aniimo for it: level 3 with the facility's personality (+20% speed). For crops and trees, the ability each job needs, in order.">?</span></th>
                                </tr>
                            </thead>
                            <tbody>${rows}</tbody>
                        </table>
                    </div>
                </div>
            `;
        }).join('');

        return `
            <div class="facility-category">
                <h4 class="facility-category-title">${category}</h4>
                ${tables}
            </div>
        `;
    }).join('');
}

window.showFacilities = async function() {
    document.getElementById('facilitiesModal').classList.add('show');
    if (recipesRendered) return;
    if (!wasmReady) {
        document.getElementById('facilities-loading-hint').textContent = 'Optimizer not ready. Please wait...';
        return;
    }
    try {
        const recipesJson = await callWorker('get_all_items');
        const recipes = JSON.parse(recipesJson);
        renderRecipeTables(recipes);
        recipesRendered = true;
        document.getElementById('facilities-loading-hint').style.display = 'none';
    } catch (error) {
        console.error('Failed to load recipe data:', error);
        document.getElementById('facilities-loading-hint').textContent = 'Failed to load recipe data. Please refresh the page.';
    }
}

window.closeFacilities = function() {
    document.getElementById('facilitiesModal').classList.remove('show');
}

window.closeFacilitiesOnBackdrop = function(event) {
    if (event.target.id === 'facilitiesModal') {
        closeFacilities();
    }
}

// Event listeners
document.addEventListener('DOMContentLoaded', () => {
    const savedData = readStorage();
    initFacilityTiers(savedData);
    renderFacilityCards();
    populateHomeLevels();
    populateLevelUpTargets();
    loadInputsFromStorage(savedData);
    attachAutoSave();
    attachFacilityTierHandlers();
    attachModeHandlers();
    attachStrategyHandlers();
    attachSkipHandlers();
    renderSkippedRecipes();
    attachSpecialHandlers();
    renderSpecialRecipes();
    attachPriorityHandlers();
    applyConfigMode();
    initWasm();

    document.getElementById('optimize-btn').addEventListener('click', runFindPlan);
    document.getElementById('clear-saved-btn').addEventListener('click', clearSavedInputs);
    document.getElementById('rate-unit').addEventListener('change', () => {
        rateUnitChosen = true;
        updateRateUnitDisplays();
    });
    document.getElementById('aniimo-best').addEventListener('change', () => showSelectedPlan(false));
    document.getElementById('aniimo-toggle').addEventListener('click', () => {
        const toggle = document.getElementById('aniimo-toggle');
        const expanded = toggle.getAttribute('aria-expanded') !== 'true';
        toggle.setAttribute('aria-expanded', String(expanded));
        document.getElementById('aniimo-body').hidden = !expanded;
    });
    document.getElementById('aniimo-minimum').addEventListener('change', () => showSelectedPlan(false));

    // Goal fields update live; no need to re-run the facility-allocation solve just because the
    // goal amount changed.
    document.getElementById('target-amount').addEventListener('input', runTimeToGoal);
    document.getElementById('current-amount').addEventListener('input', runTimeToGoal);
    document.getElementById('goal-target').addEventListener('change', runTimeToGoal);

    // Allow Enter key to trigger a full plan recalculation; but not in the goal fields, which
    // already update live on every keystroke via the listeners above. Facility tier inputs are
    // excluded here since they're already covered by the delegated listener in
    // `attachFacilityTierHandlers` (their rows come and go, so a per-element listener attached
    // once at startup wouldn't reach a tier added later).
    document.querySelectorAll('input').forEach(input => {
        if (input.id === 'target-amount' || input.id === 'current-amount') return;
        if (input.closest('#facilities-grid') || input.closest('#level-up-stock-grid') || input.id === 'skip-input') return;
        input.addEventListener('keypress', (e) => {
            if (e.key === 'Enter') {
                runFindPlan();
            }
        });
    });
});
