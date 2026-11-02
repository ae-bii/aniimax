// Tests for whether the page offers a season at all (see `SEASON` in web/facility-config.js). The
// Harvest Moon Festival is over, so it must stay off the page while its data stays for reference.
import assert from 'node:assert/strict';
import test from 'node:test';
import { readFile } from 'node:fs/promises';
import { runInNewContext } from 'node:vm';

import { SEASON } from '../web/facility-config.js';

// `seasonAvailable` from app.js, which has no page of its own, run against a season and an RV level.
const appSource = await readFile(new URL('../web/app.js', import.meta.url), 'utf8');
const start = appSource.indexOf('function seasonAvailable()');
const end = appSource.indexOf('function seasonActive()', start);
assert.ok(start >= 0 && end > start, 'seasonAvailable should still be in app.js');
const source = appSource.slice(start, end);

function seasonAvailable(season, { simple, homeLevel }) {
    const context = { SEASON: season, isSimpleMode: () => simple, selectedHomeLevel: () => homeLevel };
    runInNewContext(`${source}\nresult = seasonAvailable();`, context);
    return context.result;
}

test('a season that is over never reaches the page', () => {
    assert.equal(SEASON.over, true, 'the Harvest Moon Festival has ended');
    for (const homeLevel of [1, 9, 10, 15, 20]) {
        for (const simple of [true, false]) {
            assert.equal(seasonAvailable(SEASON, { simple, homeLevel }), false,
                `RV ${homeLevel}${simple ? '' : ' in advanced mode'}`);
        }
    }
});

test('clearing over brings a season back at its RV level', () => {
    const running = { ...SEASON, over: false };
    assert.equal(seasonAvailable(running, { simple: true, homeLevel: SEASON.minHomeLevel - 1 }), false);
    assert.equal(seasonAvailable(running, { simple: true, homeLevel: SEASON.minHomeLevel }), true);
    // Advanced mode has no RV level to go by, so a running season is always on offer there.
    assert.equal(seasonAvailable(running, { simple: false, homeLevel: 1 }), true);
});

test('the festival keeps its data while it is over', () => {
    assert.deepEqual(SEASON.recipeNotes.map(r => r.name),
        ['harvest_platter', 'umbral_pickle', 'umbral_hot_pot', 'umbral_sweet_and_spicy_sauce']);
    assert.equal(SEASON.name, 'Harvest Moon Festival');
    assert.equal(SEASON.currency, 'Moonray Wheat');
    assert.equal(SEASON.points, 'Harvest Moon Points');
    assert.equal(SEASON.minHomeLevel, 10);
});

test('the festival crops and recipes stay in the data', async () => {
    const csv = await readFile(new URL('../data/harvest_moon_festival.csv', import.meta.url), 'utf8');
    const names = csv.split('\n').slice(1).map(line => line.split(',')[0].trim()).filter(Boolean);
    assert.deepEqual(names, [
        'moondew_radish', 'waxing_moon_pepper', 'roasted_waxing_moon_pepper', 'moondew_radish_slices',
        'umbral_hot_pot', 'umbral_sweet_and_spicy_sauce', 'umbral_pickle', 'harvest_platter',
    ]);
});
