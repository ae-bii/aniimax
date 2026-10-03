import assert from 'node:assert/strict';
import test from 'node:test';

import { createShareUrl, readShareHash, urlWithoutShare } from '../web/share-config.js';

const setup = {
    facilityTiers: { Farmland: [{ count: 4, level: 2 }] },
    priorities: [{ target: 'coins', on: true }],
    roster: [{ name: 'Тёплый Анимо 🌾', count: 1 }],
    'mode-simple': false,
};

test('test_share_link_round_trips_unicode_setup', async () => {
    const url = new URL(await createShareUrl('https://example.com/aniimax/?view=home', setup));
    assert.equal(url.pathname, '/aniimax/');
    assert.equal(url.search, '?view=home');
    assert.equal(url.searchParams.has('config'), false);
    assert.deepEqual(await readShareHash(url.hash), setup);
    assert.equal(urlWithoutShare(url.href), 'https://example.com/aniimax/?view=home');
});

test('test_share_link_ignores_unrelated_fragments_and_rejects_invalid_data', async () => {
    assert.equal(await readShareHash('#help'), null);
    await assert.rejects(readShareHash('#config=v2.abcd'));
    await assert.rejects(readShareHash('#config=v1.not-valid!'));
    await assert.rejects(readShareHash('#config=v1.e30'));
    assert.equal(urlWithoutShare('https://example.com/#section=help&config=v1.e30'), 'https://example.com/#section=help');
});

test('test_share_link_rejects_oversized_setup', async () => {
    await assert.rejects(createShareUrl('https://example.com/', {
        facilityTiers: {},
        note: 'a'.repeat(1_000_000),
    }));
});

test('test_share_link_compresses_repeated_data_losslessly', async () => {
    const fullSetup = { ...setup, roster: Array.from({ length: 50 }, (_, i) => ({
        name: `Aniimo ${i}`,
        count: 2,
        abilities: { Gathering: 3, Crafting: 2 },
    })) };
    const url = new URL(await createShareUrl('https://example.com/', fullSetup));
    assert.ok(url.href.length < JSON.stringify(fullSetup).length);
    assert.deepEqual(await readShareHash(url.hash), fullSetup);
});
