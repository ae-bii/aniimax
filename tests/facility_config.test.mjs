import assert from 'node:assert/strict';
import test from 'node:test';

import { simpleSetup } from '../web/facility-config.js';

const countAt = (homeLevel, facility) => simpleSetup(homeLevel).facilities[facility][0].count;

test('RV 14 applies the confirmed facility count increases', () => {
    const expected = {
        Well: 3,
        'Tidewhisper Sandcastle': 2,
        'Woodworking Bench': 3,
        'Chimney Kiln': 3,
        'Joy Wheel Loom': 2,
    };

    for (const [facility, count] of Object.entries(expected)) {
        assert.equal(countAt(13, facility), count - 1, `${facility} should keep its RV 13 count`);
        assert.equal(countAt(14, facility), count, `${facility} should increase at RV 14`);
        assert.equal(countAt(15, facility), count, `${facility} should keep its RV 14 count at RV 15`);
    }
});

test('later RV levels apply confirmed maximum facility counts', () => {
    const expectedByLevel = {
        15: { 'Blazing Stove': 2, 'Pickling Jar': 2 },
        16: { 'Nimbus Bed': 2 },
        17: { 'Cooling Unit': 3, 'Dewy House': 2, 'Heat Furnace': 3 },
        18: { 'Chimney Kiln': 4, 'Starfall Hammock': 2, 'Woodworking Bench': 4 },
        19: { Sunlamp: 3, Well: 4 },
    };

    for (const [levelText, facilities] of Object.entries(expectedByLevel)) {
        const level = Number(levelText);
        for (const [facility, count] of Object.entries(facilities)) {
            assert.equal(countAt(level - 1, facility), count - 1, `${facility} should keep its previous maximum through RV ${level - 1}`);
            assert.equal(countAt(level, facility), count, `${facility} should increase at RV ${level}`);
            assert.equal(countAt(20, facility), count, `${facility} should keep its confirmed maximum afterward`);
        }
    }

});
