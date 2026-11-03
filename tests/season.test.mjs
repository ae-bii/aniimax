// Tests for which seasons the page offers (see `SEASONS` in web/facility-config.js). An event that
// has ended (`over`) must stay off the page, and out of plans, while its data stays for reference.
import assert from 'node:assert/strict';
import test from 'node:test';
import { readFile } from 'node:fs/promises';
import { runInNewContext } from 'node:vm';

import { SEASONS, RUNNING_SEASONS, DEFAULT_SEASON, runningSeason, seasonById } from '../web/facility-config.js';

const appSource = await readFile(new URL('../web/app.js', import.meta.url), 'utf8');

// The source of one function from app.js, which has no page of its own, from its `function`
// line up to the next marker.
function appFunction(name, next) {
    const start = appSource.indexOf(`function ${name}(`);
    const end = appSource.indexOf(next, start);
    assert.ok(start >= 0 && end > start, `${name} should still be in app.js`);
    return appSource.slice(start, end);
}

// `seasonAvailable` run against the season the selector resolves to (`null` with none running).
function seasonAvailable(season, { simple, homeLevel }) {
    const context = { currentSeason: () => season, isSimpleMode: () => simple, selectedHomeLevel: () => homeLevel };
    runInNewContext(`${appFunction('seasonAvailable', 'function seasonActive()')}\nresult = seasonAvailable();`, context);
    return context.result;
}

const harvestMoon = seasonById('harvest_moon_festival');

test('an ended season is never offered or planned with', () => {
    assert.equal(harvestMoon.over, true, 'the Harvest Moon Festival has ended');
    assert.ok(!RUNNING_SEASONS.includes(harvestMoon));
    // A stale pick from an old save or a shared link falls back to a running season.
    assert.notEqual(runningSeason('harvest_moon_festival')?.id, 'harvest_moon_festival');
    assert.equal(runningSeason('harvest_moon_festival').id, DEFAULT_SEASON);
    assert.equal(runningSeason('no_such_event').id, DEFAULT_SEASON);
});

test('the default season is running', () => {
    assert.equal(runningSeason(DEFAULT_SEASON).id, DEFAULT_SEASON);
    assert.ok(!seasonById(DEFAULT_SEASON).over);
});

test('with no season running the section is off', () => {
    for (const homeLevel of [1, 10, 20]) {
        for (const simple of [true, false]) {
            assert.equal(seasonAvailable(null, { simple, homeLevel }), false);
        }
    }
});

test('a running season is offered from its RV level', () => {
    const season = runningSeason(DEFAULT_SEASON);
    assert.equal(seasonAvailable(season, { simple: true, homeLevel: season.minHomeLevel - 1 }), false);
    assert.equal(seasonAvailable(season, { simple: true, homeLevel: season.minHomeLevel }), true);
    // Advanced mode has no RV level to go by, so a running season is always on offer there.
    assert.equal(seasonAvailable(season, { simple: false, homeLevel: 1 }), true);
});

test('every season has what the page shows of it', () => {
    for (const season of SEASONS) {
        for (const key of ['id', 'name', 'currency', 'points', 'hint', 'budgetLabel']) {
            assert.equal(typeof season[key], 'string', `${season.id ?? '?'} ${key}`);
        }
        assert.ok(Array.isArray(season.recipeNotes));
    }
});

test('the festival keeps its data while it is over', () => {
    assert.deepEqual(harvestMoon.recipeNotes.map(r => r.name),
        ['harvest_platter', 'umbral_pickle', 'umbral_hot_pot', 'umbral_sweet_and_spicy_sauce']);
    assert.equal(harvestMoon.name, 'Harvest Moon Festival');
    assert.equal(harvestMoon.currency, 'Moonray Wheat');
    assert.equal(harvestMoon.points, 'Harvest Moon Points');
    assert.equal(harvestMoon.minHomeLevel, 10);
});

test('the festival crops and recipes stay in the data', async () => {
    const csv = await readFile(new URL('../data/harvest_moon_festival.csv', import.meta.url), 'utf8');
    const names = csv.split('\n').slice(1).map(line => line.split(',')[0].trim()).filter(Boolean);
    assert.deepEqual(names, [
        'moondew_radish', 'waxing_moon_pepper', 'roasted_waxing_moon_pepper', 'moondew_radish_slices',
        'umbral_hot_pot', 'umbral_sweet_and_spicy_sauce', 'umbral_pickle', 'harvest_platter',
    ]);
});

// `migrateSavedConfig` run on a saved config blob.
function migrate(data) {
    const context = { data };
    runInNewContext(`${appFunction('migrateSavedConfig', '// Populates the module-level')}\nresult = migrateSavedConfig(data);`, context);
    // Objects made inside the context have its own prototypes; a JSON round trip brings them home.
    return JSON.parse(JSON.stringify(context.result));
}

test('a single saved budget goes to the season it was typed for', () => {
    // From before the picker: the only season was the Harvest Moon Festival.
    assert.deepEqual(migrate({ 'season-currency-per-day': '500' }).seasonBudgets, { harvest_moon_festival: '500' });
    // From with the picker but one shared field: the season picked then.
    assert.deepEqual(migrate({ 'season-currency-per-day': '40', 'season-event': 'magical_wonder_night' }).seasonBudgets,
        { magical_wonder_night: '40' });
    // Budgets already kept per season stay as they are.
    assert.deepEqual(migrate({ 'season-currency-per-day': '500', seasonBudgets: { magical_wonder_night: '12' } }).seasonBudgets,
        { magical_wonder_night: '12' });
});
