// Aniimax Web Application

import {
    FACILITIES, FACILITY_CATEGORIES, FACILITY_CATEGORY_BY_NAME,
    MAX_HOME_LEVEL, COUNTS_CONFIRMED_UP_TO, ANIIMO_MAX, simpleSetup,
    LEVEL_UP_COSTS, LEVEL_UP_CHAINS,
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
};

// Multiplier from the solver's native per-second rate to each display unit, and the short suffix
// shown next to the currency label (e.g. "Coins/hour"). "Your Rate" is stored and computed
// per-second throughout; this only affects how that one number is displayed.
const RATE_UNIT_SECONDS = {
    second: { multiplier: 1, suffix: '/sec' },
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

// Every plain input ID whose value should be persisted (facility tiers are saved separately;
// see `facilityTiers`/`initFacilityTiers`, since they're a dynamic list rather than one fixed
// element per facility).
function getPersistedFieldIds() {
    return [
        'target-amount', 'current-amount',
        'prioritize-byproducts',
        'strategy-level-up', 'strategy-coins', 'level-up-target',
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
    const data = { facilityTiers, levelUpStock };
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

        console.log(`Aniimax v${version} loaded successfully`);
    } catch (error) {
        console.error('Failed to initialize WASM:', error);
        showError('Failed to load the optimizer. Please refresh the page.');
    }
}

// --- Simple / advanced mode -----------------------------------------------------------
// Simple mode takes just the RV (Homeland) level and assumes everything that level allows is built
// and upgraded (see `simpleSetup` in facility-config.js). Advanced mode is the full per-facility
// input. Switching modes never overwrites the advanced inputs; "Customize in advanced mode" copies
// the simple setup into them on purpose.

function isSimpleMode() {
    return document.getElementById('mode-simple').checked;
}

function selectedHomeLevel() {
    return numberOrDefault(document.getElementById('home-level').value, MAX_HOME_LEVEL);
}

function populateHomeLevels() {
    const select = document.getElementById('home-level');
    const options = [];
    for (let level = 1; level <= MAX_HOME_LEVEL; level++) {
        options.push(`<option value="${level}">${level}${level === MAX_HOME_LEVEL ? ' (everything unlocked)' : ''}</option>`);
    }
    select.innerHTML = options.join('');
    select.value = String(MAX_HOME_LEVEL);
}

// One entry per built facility, e.g. "10 Farmland Lv.2", plus a note when counts above the
// confirmed RV levels are estimates.
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
    const notes = homeLevel > COUNTS_CONFIRMED_UP_TO
        ? `<ul class="assume-notes">
               <li>Building counts are confirmed up to RV level ${COUNTS_CONFIRMED_UP_TO}; above that they're estimates.</li>
               <li>Facility levels past RV level ${COUNTS_CONFIRMED_UP_TO} haven't been checked in game yet.</li>
           </ul>`
        : '';
    document.getElementById('simple-summary').innerHTML = `
        <p class="assume-title">Facilities</p>
        <div class="chip-grid">${built}</div>
        <p class="assume-title">Modules</p>
        <div class="chip-grid">${moduleChips}</div>
        ${notes}`;
}

function applyConfigMode() {
    const simple = isSimpleMode();
    document.getElementById('simple-config').style.display = simple ? 'block' : 'none';
    document.getElementById('advanced-config').style.display = simple ? 'none' : 'block';
    if (simple) renderSimpleSummary();
    renderStrategy();
}

// Copies the simple-mode setup into the advanced inputs and switches to advanced mode, so the
// player can start from "everything at my RV level" and adjust from there.
function customizeInAdvancedMode() {
    const { facilities, modules } = simpleSetup(selectedHomeLevel());
    FACILITIES.forEach(f => {
        facilityTiers[f.name] = facilities[f.name].map(t => ({ ...t }));
        renderTierRows(f.name);
    });
    document.getElementById('ecological-module-level').value = modules.ecological_module;
    document.getElementById('kitchen-module-level').value = modules.kitchen_module;
    document.getElementById('resource-detector-level').value = modules.resource_detector;
    document.getElementById('crafting-module-level').value = modules.crafting_module;
    document.getElementById('mode-advanced').checked = true;
    applyConfigMode();
    saveInputsToStorage();
    document.getElementById('advanced-config').scrollIntoView({ behavior: 'smooth' });
}

function attachModeHandlers() {
    document.getElementById('mode-simple').addEventListener('change', applyConfigMode);
    document.getElementById('mode-advanced').addEventListener('change', applyConfigMode);
    document.getElementById('home-level').addEventListener('change', () => {
        renderSimpleSummary();
        renderStrategy();
    });
    document.getElementById('customize-btn').addEventListener('click', customizeInAdvancedMode);
}

// --- Strategy ----------------------------------------------------------------------------
// "Level up" plans the soonest next RV level-up (its coins plus a Woodworking Bench item and a
// Chimney Kiln item, less what's already in stock); "Most coins" plans the most coins.

// What the player has toward a level-up, by item name ('coins' for coins).
let levelUpStock = {};

const ITEM_NAMES = {
    coins: 'Coins',
    wood_block: 'Wood Blocks',
    mineral_sand: 'Mineral Sand',
    coarse_sifted_ore: 'Coarse-Sifted Ore',
};

function isLevelUpStrategy() {
    return document.getElementById('strategy-level-up').checked;
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
    if (!LEVEL_UP_COSTS[target]) return `Level-up costs are only known from RV 7 on.`;
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
    document.getElementById('coins-config').style.display = levelUp ? 'none' : 'block';
    if (!levelUp) return;

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
    document.getElementById('strategy-coins').addEventListener('change', renderStrategy);
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

// The level-up card: how soon the plan covers the target's cost, one line per cost.
function renderLevelUp(plan) {
    const card = document.getElementById('level-up-card');
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
    const slowest = Math.max(...report.requirements.map(r => r.seconds ?? Infinity));
    const rows = report.requirements.map(r => {
        const ready = r.seconds === null ? 'never' : r.seconds === 0 ? 'have it' : formatDuration(r.seconds);
        const isSlowest = r.seconds !== null && r.seconds > 0 && r.seconds >= slowest * (1 - 1e-6);
        return `<tr${isSlowest ? ' class="slowest"' : ''}>
            <td>${ITEM_NAMES[r.name] || prettyItem(r.name)}</td>
            <td>${formatNumber(r.need)}</td>
            <td>${formatNumber(r.have)}</td>
            <td>${perHour(r.per_second)}</td>
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
            <thead><tr><th>Cost</th><th>Need</th><th>Have</th><th>Per hour</th><th>Ready in</th></tr></thead>
            <tbody>${rows}</tbody>
        </table>
        ${coinsNote}`;
}

// What each product sold earns in a level-up plan, per hour and by the time the level-up is
// ready. (Most coins plans show this in the goal card instead.)
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
            <td data-label="By the level-up">${formatNumber(Math.floor(s.rate_per_second * report.seconds))}</td>
        </tr>`).join('');
    document.getElementById('profit-breakdown').innerHTML = `
        <div class="table-wrapper">
            <table class="facility-plan-table">
                <thead><tr><th>Product</th><th>Facility</th><th>Sold per hour</th><th>Profit per hour</th><th>Share</th><th>By the level-up</th></tr></thead>
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
            prioritize_byproducts: !isLevelUpStrategy() && document.getElementById('prioritize-byproducts').checked,
            level_up: levelUpInput(),
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
        prioritize_byproducts: !isLevelUpStrategy() && document.getElementById('prioritize-byproducts').checked,
        level_up: levelUpInput(),
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

// Updates the goal section's labels ("Target Coins"/"Coins Produced" etc.) to match the plan's
// currency, so the labels never drift out of sync with what's actually being calculated.
function updateCurrencyLabels(currency) {
    const label = CURRENCY_LABELS[currency] || currency;
    document.getElementById('target-amount-label').textContent = `Target ${label}`;
    document.getElementById('current-amount-label').textContent = `Current ${label}`;
    document.getElementById('amount-produced-label').textContent = `${label} Produced`;
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
            <td>${formatNumber(p.rate_per_second * multiplier)}</td>
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
                <tbody>${rows.map(step => `
                    <tr class="status-${step.status}">
                        <td data-label="Facility">${step.facility}</td>
                        <td data-label="Count">${step.facility_count}</td>
                        <td data-label="Producing">${step.item_name ? prettyItem(step.item_name) : '-'}${unverifiedRowKeys.has(`${step.facility}|${step.item_name}`) ? '<span class="tag unverified" title="Not yet checked in game">unverified</span>' : ''}</td>
                        <td data-label="Aniimo">${aniimoLabel(step)}</td>
                        <td data-label="Why">${prettyReason(step.reason)}</td>
                    </tr>
                `).join('')}</tbody>
            </table>
        </div>
    `;
}

// The Aniimo team the shown plan needs, one row per distinct ability / level / personality.
// Aniimo move between any jobs they can do, so each row needs enough of them to cover the work
// on average (a facility waiting on ingredients frees its Aniimo), rounded up. Facilities with a
// resident Aniimo (Sandcastle and the like) are always busy, so they count one each.
function renderAniimoSummary(plan) {
    const container = document.getElementById('aniimo-summary');
    const groups = new Map();
    (plan.coin_items || []).forEach(step => {
        (step.aniimo_tasks || []).forEach(task => {
            const key = taskLabel(task, step.facility);
            if (!groups.has(key)) {
                groups.set(key, { label: key, ability: task.ability, level: task.level, bonus: task.personality_bonus, busy: 0, where: new Map() });
            }
            const g = groups.get(key);
            g.busy += task.busy;
            const place = `${step.facility} (${prettyItem(step.item_name)})`;
            g.where.set(place, (g.where.get(place) || 0) + step.facility_count);
        });
    });
    // Environment buildings in use each keep an Aniimo busy (abilities confirmed in game; whether
    // level or personality matters isn't known yet, so any level is shown).
    (plan.environment_assignments || []).forEach(a => {
        const ability = ENVIRONMENT_BUILDING_ABILITY[a.building];
        if (!ability || !a.units) return;
        const key = `${ability} (environment)`;
        if (!groups.has(key)) {
            groups.set(key, { label: `${ability} any level`, ability, level: 1, bonus: false, busy: 0, where: new Map(), environment: true });
        }
        const g = groups.get(key);
        g.busy += a.units;
        const place = `${a.building} (${a.mode})`;
        g.where.set(place, (g.where.get(place) || 0) + a.units);
    });
    const collapsedSummary = document.getElementById('aniimo-collapsed-summary');
    if (groups.size === 0) {
        container.innerHTML = '<p class="hint">Nothing in this plan needs an Aniimo.</p>';
        collapsedSummary.textContent = 'No Aniimo needed.';
        document.getElementById('aniimo-abilities').innerHTML = '';
        return;
    }
    // An Aniimo can do any job of its ability at or below its level, so work that fits in a
    // higher-level row's spare time (e.g. Farmland jobs, which take any level) joins that row
    // instead of calling for another Aniimo. Rows that count on a personality bonus stay separate.
    const sorted = [...groups.values()].sort((a, b) => b.level - a.level || Number(b.bonus) - Number(a.bonus) || a.label.localeCompare(b.label));
    const kept = [];
    sorted.forEach(g => {
        const host = g.bonus || g.environment ? null : kept.find(k => k.ability === g.ability && k.level >= g.level && k.spare >= g.busy - 1e-6);
        if (host) {
            host.spare -= g.busy;
            host.busy += g.busy;
            g.where.forEach((n, place) => host.where.set(place, (host.where.get(place) || 0) + n));
            return;
        }
        g.count = Math.max(1, Math.ceil(g.busy - 1e-6));
        g.spare = g.count - g.busy;
        kept.push(g);
    });
    let total = 1; // the Hauling row below
    const rows = kept
        .sort((a, b) => a.label.localeCompare(b.label))
        .map(g => {
            total += g.count;
            const where = [...g.where.entries()].map(([place, n]) => `${n > 1 ? n + '× ' : ''}${place}`).join(', ');
            const rest = g.label.slice(g.ability.length).trim();
            return `<tr><td data-label="Aniimo">${abilityTag(g.ability)} ${rest}</td><td data-label="How many">${g.count}</td><td data-label="Busy on average">${g.busy.toFixed(1)}</td><td data-label="Where">${where}</td></tr>`;
        })
        .join('');
    const haulingRow = `<tr><td data-label="Aniimo">${abilityTag('Hauling')} any level</td><td data-label="How many">1+</td><td data-label="Busy on average">?</td><td data-label="Where">Carries produce to storage. How much work this is isn't known yet; add more if produce piles up.</td></tr>`;

    let capNote = '';
    const cap = isSimpleMode() ? ANIIMO_MAX[selectedHomeLevel() - 1] : null;
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
        const where = [...g.where.entries()].map(([place, n]) => `${n > 1 ? n + '× ' : ''}${place}`).join(', ');
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
            units.push({ building: a.building, remaining, rows: [], layout });
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
    'Farmland': '#c9a24d',
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
const ENVIRONMENT_MODE_COLORS = {
    Warm: '#f59e0b',
    Scorching: '#ef4444',
    Cool: '#60a5fa',
    Freeze: '#67e8f9',
    Adequate: '#facc15',
};

// Renders one building's layout as an SVG: faint one-tile gridlines, the building, its coverage
// area shaded in the environment's color, and every plot the plan puts in it, nearest the
// building first. `rows` are this building's plan rows; each plot is matched to one of them so
// hovering a plot names its crop, and when one facility type grows more than one crop here (so
// color alone can't tell them apart) each plot shows its crop's number from the legend.
// Positions are the solver's own, in game tiles.
function renderEnvironmentDiagram(layout, mode, building, rows = []) {
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

    // Gridlines like the game's: stronger on whole tiles, very faint on the quarter tiles
    // facilities snap to.
    const gridLines = [];
    for (let t = Math.ceil(viewMin * 4) / 4; t <= viewMin + viewSize; t += 0.25) {
        const cls = Number.isInteger(t) ? 'tile' : 'quarter';
        gridLines.push(`<line class="${cls}" x1="${t}" y1="${viewMin}" x2="${t}" y2="${viewMin + viewSize}" />`);
        gridLines.push(`<line class="${cls}" x1="${viewMin}" y1="${t}" x2="${viewMin + viewSize}" y2="${t}" />`);
    }

    // Plots nearest the building first, each matched to a plan row of its facility type.
    const distance = p => Math.hypot(p.x + p.size / 2 - buildingCenter, p.y + p.size / 2 - buildingCenter);
    const plots = [...layout].sort((a, b) => distance(a) - distance(b));
    const queue = {};
    rows.forEach(r => {
        if (!r.item_name) return;
        (queue[r.facility] = queue[r.facility] || []).push({ item: r.item_name, left: r.facility_count });
    });
    const cropOf = p => {
        const q = queue[p.facility];
        while (q && q.length && q[0].left <= 0) q.shift();
        if (!q || !q.length) return null;
        q[0].left--;
        return q[0].item;
    };
    const assigned = plots.map(p => ({ ...p, crop: cropOf(p) }));
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

    // Legend: the coverage, then each crop with how many plots it gets here.
    const counts = {};
    assigned.forEach(p => {
        const key = `${p.facility}|${p.crop}`;
        counts[key] = (counts[key] || 0) + 1;
    });
    const legend = [`
        <span class="env-legend-item">
            <span class="env-legend-swatch coverage" style="background:${tint}33;border-color:${tint}"></span>${mode} coverage
        </span>`].concat(Object.entries(counts).map(([key, n]) => {
        const [facility, crop] = key.split('|');
        const name = crop && crop !== 'null' ? `${facility}: ${prettyItem(crop)}` : facility;
        return `
        <span class="env-legend-item">
            <span class="env-legend-swatch" style="background:${ENVIRONMENT_FACILITY_COLORS[facility] || '#888888'}"></span>${numbered ? `<b>${numberOf(key)}</b> ` : ''}${name} ×${n}
        </span>`;
    })).join('');

    return `
        <div class="env-diagram">
            <svg viewBox="${viewMin} ${viewMin} ${viewSize} ${viewSize}" role="img" aria-label="${building} layout, ${mode} coverage">
                <g class="env-grid">${gridLines.join('')}</g>
                <rect x="${coverageMin}" y="${coverageMin}" width="${coverageSize}" height="${coverageSize}"
                      fill="${tint}" fill-opacity="0.12" stroke="${tint}" stroke-opacity="0.8" stroke-dasharray="0.35,0.25" stroke-width="0.08" />
                ${rects}
                <g class="env-building"><title>${building} (${mode})</title>
                    <rect x="0.05" y="0.05" width="${ENVIRONMENT_BUILDING_SIZE - 0.1}" height="${ENVIRONMENT_BUILDING_SIZE - 0.1}" rx="0.3" fill="${tint}" stroke="currentColor" stroke-opacity="0.6" stroke-width="0.08" />
                </g>
            </svg>
            <div class="env-legend">${legend}</div>
            <p class="env-note">A plot counts as covered if any part of it is inside the dashed area.</p>
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
    const environmentSections = ENVIRONMENT_MODE_ORDER.filter(mode => envGroups.has(mode)).map(mode => {
        const assignmentsForMode = assignments.filter(a => a.mode === mode);
        const units = splitByEnvironmentUnit(envGroups.get(mode), assignmentsForMode);

        const unitTables = units.length === 0
            ? facilityPlanTable(envGroups.get(mode))
            : units.map((unit, i) => `
                ${units.length > 1 ? `<p class="hint small">${unit.building} ${i + 1}</p>` : ''}
                <div class="env-unit">
                    ${renderEnvironmentDiagram(unit.layout, mode, unit.building, unit.rows)}
                    <div class="env-unit-table">${facilityPlanTable(unit.rows)}</div>
                </div>
            `).join('');

        return `
            <div class="facility-category">
                <h4 class="facility-category-title">Environment: ${mode}</h4>
                ${unitTables}
            </div>
        `;
    }).join('');

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
// switching units never needs a facility-allocation re-solve.
function updateRateDisplay() {
    if (!lastPlan || !lastPlan.success) return;
    const unit = document.getElementById('rate-unit').value;
    const { multiplier, suffix } = RATE_UNIT_SECONDS[unit] || RATE_UNIT_SECONDS.second;
    const label = CURRENCY_LABELS[lastPlan.currency] || lastPlan.currency;
    document.getElementById('plan-rate').textContent =
        `${formatNumber(lastPlan.rate_per_second * multiplier)} ${label}${suffix}`;
}

// Re-renders every rate-unit-dependent display ("Your Rate" and the Product Breakdown table's
// Profit column) from the already-computed `lastPlan`/`lastGoalResult`; the `#rate-unit` change
// listener target, so switching units never needs a re-solve.
function updateRateUnitDisplays() {
    updateRateDisplay();
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

    updateRateDisplay();
    updateCurrencyLabels(plan.currency);

    const explored = document.getElementById('plan-explored-hint');
    if (plan.proven_optimal === true && plan.level_up) {
        explored.innerHTML = `<span class="badge">✓ Proven best plan</span>No other plan gets RV ${planContext.target} sooner or earns more on the way, for the game data we have.`;
    } else if (plan.proven_optimal === true) {
        explored.innerHTML = '<span class="badge">✓ Proven best plan</span>No other use of these facilities earns more, for the game data we have.';
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
    document.getElementById('total-time').textContent = goalResult.total_time_formatted;
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
    if (!lastPlan || !lastPlan.success) return;

    const target = floatOrDefault(document.getElementById('target-amount').value, 0);
    const current = floatOrDefault(document.getElementById('current-amount').value, 0);

    try {
        const resultJson = await callWorker('time_to_reach', JSON.stringify({ plan: lastPlan, target, current }));
        displayGoal(JSON.parse(resultJson));
    } catch (error) {
        console.error('Goal calculation error:', error);
    }
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
        const jobs = recipe.jobs || [];
        if (jobs.length === 0) return '-';
        return `<span class="job-list">${jobs.map(([step, ability, level]) =>
            `<span class="job"><span class="job-step">${step}</span> ${abilityTag(ability)}${level > 1 ? ` Lv.${level}+` : ''}</span>`).join('')}</span>`;
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
                    <td class="recipe-name">${prettyItem(r.name)}${r.verified === false ? ' <span class="info-icon" data-tooltip="Not yet checked in game.">?</span>' : ''}</td>
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
                                    <th>Time <span class="info-icon" data-tooltip="Grow time for crops and trees. Everything else lists workload: how long it takes depends on the Aniimo working it (108 workload takes 108s at level 1, 36s at level 2, 27s at level 3).">?</span></th>
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
    applyConfigMode();
    initWasm();

    document.getElementById('optimize-btn').addEventListener('click', runFindPlan);
    document.getElementById('clear-saved-btn').addEventListener('click', clearSavedInputs);
    document.getElementById('rate-unit').addEventListener('change', updateRateUnitDisplays);
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

    // Allow Enter key to trigger a full plan recalculation; but not in the goal fields, which
    // already update live on every keystroke via the listeners above. Facility tier inputs are
    // excluded here since they're already covered by the delegated listener in
    // `attachFacilityTierHandlers` (their rows come and go, so a per-element listener attached
    // once at startup wouldn't reach a tier added later).
    document.querySelectorAll('input').forEach(input => {
        if (input.id === 'target-amount' || input.id === 'current-amount') return;
        if (input.closest('#facilities-grid') || input.closest('#level-up-stock-grid')) return;
        input.addEventListener('keypress', (e) => {
            if (e.key === 'Enter') {
                runFindPlan();
            }
        });
    });
});
