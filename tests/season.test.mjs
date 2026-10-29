// Tests for the seasons the page knows (see `SEASONS` in web/facility-config.js): which one, if
// any, reaches the page, and that the three registries of seasons — the web config, src/data.rs
// and src/wasm.rs — name the same events as the data directory.
import assert from 'node:assert/strict';
import test from 'node:test';
import { readFile, readdir } from 'node:fs/promises';
import { runInNewContext } from 'node:vm';

import { SEASONS, CURRENT_SEASON, seasonById } from '../web/facility-config.js';

// `seasonAvailable` from app.js, which has no page of its own, run against a season and an RV level.
const appSource = await readFile(new URL('../web/app.js', import.meta.url), 'utf8');
const start = appSource.indexOf('function seasonAvailable()');
const end = appSource.indexOf('function seasonActive()', start);
assert.ok(start >= 0 && end > start, 'seasonAvailable should still be in app.js');
const source = appSource.slice(start, end);

function seasonAvailable(season, { simple, homeLevel }) {
    const context = { CURRENT_SEASON: season, isSimpleMode: () => simple, selectedHomeLevel: () => homeLevel };
    runInNewContext(`${source}\nresult = seasonAvailable();`, context);
    return context.result;
}

test('at most one season runs at a time, and it is the current one', () => {
    const running = SEASONS.filter(season => !season.over);
    assert.ok(running.length <= 1, `${running.length} seasons are not over`);
    assert.equal(CURRENT_SEASON, running[0] ?? null);
});

test('no season reaches the page between events', () => {
    for (const homeLevel of [1, 9, 10, 15, 20]) {
        for (const simple of [true, false]) {
            assert.equal(seasonAvailable(null, { simple, homeLevel }), false,
                `RV ${homeLevel}${simple ? '' : ' in advanced mode'}`);
        }
    }
});

test('a running season reaches the page at its own RV level', () => {
    const running = { id: 'test_festival', name: 'Test Festival', minHomeLevel: 12, over: false, recipeNotes: [] };
    assert.equal(seasonAvailable(running, { simple: true, homeLevel: 11 }), false);
    assert.equal(seasonAvailable(running, { simple: true, homeLevel: 12 }), true);
    // Advanced mode has no RV level to go by, so a running season is always on offer there.
    assert.equal(seasonAvailable(running, { simple: false, homeLevel: 1 }), true);
});

test('seasons are found by id, and an unknown id finds none', () => {
    for (const season of SEASONS) assert.equal(seasonById(season.id), season);
    assert.equal(seasonById('no_such_festival'), null);
    assert.equal(seasonById(undefined), null);
    assert.equal(new Set(SEASONS.map(s => s.id)).size, SEASONS.length, 'ids are unique');
});

test('every season has the data and the wording an event needs', async () => {
    const files = new Set(await readdir(new URL('../data', import.meta.url)));
    for (const season of SEASONS) {
        assert.ok(files.has(`${season.id}.csv`), `${season.id} has no data file`);
        for (const field of ['name', 'currency', 'points']) {
            assert.equal(typeof season[field], 'string', `${season.id} has no ${field}`);
            assert.ok(season[field].length > 0, `${season.id} has an empty ${field}`);
        }
        assert.ok(Number.isInteger(season.minHomeLevel), `${season.id} has no RV level`);
        assert.ok(Array.isArray(season.recipeNotes), `${season.id} has no recipe notes`);
    }
});

test('the web, data.rs and wasm.rs all name the same seasons', async () => {
    const read = async path => readFile(new URL(path, import.meta.url), 'utf8');
    const ids = text => [...text.matchAll(/"([a-z0-9_]+)"/g)].map(match => match[1]);

    const dataRs = await read('../src/data.rs');
    const dataList = dataRs.match(/pub const SEASONS: &\[&str\] = &\[([^\]]*)\]/);
    assert.ok(dataList, 'src/data.rs should declare SEASONS');

    const wasmRs = await read('../src/wasm.rs');
    const wasmList = wasmRs.match(/const SEASONS: &\[\(&str, &str\)\] = &\[([\s\S]*?)\n\];/);
    assert.ok(wasmList, 'src/wasm.rs should declare SEASONS');
    // Each line is ("<id>", include_str!("../data/<id>.csv")): take the first string of each.
    const wasmIds = wasmList[1].split('\n').map(line => ids(line)[0]).filter(Boolean);

    const webIds = SEASONS.map(season => season.id);
    assert.deepEqual(ids(dataList[1]), webIds, 'src/data.rs lists other seasons than the web config');
    assert.deepEqual(wasmIds, webIds, 'src/wasm.rs lists other seasons than the web config');
    for (const id of webIds) {
        assert.ok(wasmRs.includes(`include_str!("../data/${id}.csv")`), `wasm.rs does not embed ${id}.csv`);
    }
});
