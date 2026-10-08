import assert from 'node:assert/strict';
import { test } from 'node:test';

import { createMathRetypeset } from '../web/math-retypeset.js';

// A language code that stands for any language other than zh-TW.
const OTHER = 'xx';

const SOURCE = '<p>Each recipe \\(r\\) gets a rate.</p>';
const TYPESET_ENGLISH = '<p>Each recipe <mjx-container>r</mjx-container> gets a rate.</p>';

// A promise and the function that settles it, so a test decides when MathJax is done.
function deferred() {
    let resolve;
    const promise = new Promise(done => { resolve = done; });
    return { promise, resolve };
}

// A page with a math window, the current language and a MathJax stand-in.
function page({ language = 'en', mathJax = null } = {}) {
    const state = { language, mathJax, translated: [] };
    const body = { innerHTML: TYPESET_ENGLISH };
    const queue = createMathRetypeset({
        body,
        source: SOURCE,
        initialLanguage: 'en',
        getLanguage: () => state.language,
        getMathJax: () => state.mathJax,
        // Marks the text with the language it was translated to.
        translateTree: element => {
            state.translated.push(state.language);
            if (state.language !== 'en') element.innerHTML = `[${state.language}]${element.innerHTML}`;
        },
    });
    return { state, body, queue };
}

// A MathJax stand-in that records its calls and can hold its startup and each typeset.
function fakeMathJax({ startup = Promise.resolve(), typesetDelay = null } = {}) {
    const calls = [];
    let running = 0;
    let mostAtOnce = 0;
    return {
        calls,
        mostAtOnce: () => mostAtOnce,
        startup: { promise: startup },
        typesetClear: elements => calls.push(['clear', elements[0].innerHTML]),
        typesetPromise: async elements => {
            running += 1;
            mostAtOnce = Math.max(mostAtOnce, running);
            calls.push(['typeset', elements[0].innerHTML]);
            if (typesetDelay) await typesetDelay();
            running -= 1;
        },
    };
}

test('a saved zh-TW waits for the MathJax startup typeset, then retypesets the source', async () => {
    const startup = deferred();
    const mathJax = fakeMathJax({ startup: startup.promise });
    const { state, body, queue } = page({ language: 'zh-TW', mathJax });
    const done = queue();
    await new Promise(resolve => setImmediate(resolve));
    assert.equal(body.innerHTML, TYPESET_ENGLISH, 'nothing changes while MathJax still typesets');
    startup.resolve();
    await done;
    assert.equal(body.innerHTML, `[zh-TW]${SOURCE}`);
    assert.deepEqual(mathJax.calls, [['clear', TYPESET_ENGLISH], ['typeset', `[zh-TW]${SOURCE}`]]);
    assert.deepEqual(state.translated, ['zh-TW']);
});

test('before MathJax loads, zh-TW still puts the translated source in place', async () => {
    const { body, queue } = page({ language: 'zh-TW', mathJax: null });
    await queue();
    assert.equal(body.innerHTML, `[zh-TW]${SOURCE}`);
});

test('a change away from zh-TW puts the source back for the new language', async () => {
    const mathJax = fakeMathJax();
    const { state, body, queue } = page({ language: 'zh-TW', mathJax });
    await queue();
    state.language = 'en';
    await queue();
    assert.equal(body.innerHTML, SOURCE);
    assert.deepEqual(mathJax.calls.at(-1), ['typeset', SOURCE]);
    state.language = OTHER;
    await queue();
    assert.equal(body.innerHTML, SOURCE, 'a change between two other languages does not touch the math window');
});

test('a session that never uses zh-TW keeps the math window as MathJax typeset it', async () => {
    const mathJax = fakeMathJax();
    const { state, body, queue } = page({ language: OTHER, mathJax });
    await queue();
    state.language = 'en';
    await queue();
    assert.equal(body.innerHTML, TYPESET_ENGLISH);
    assert.deepEqual(mathJax.calls, []);
    assert.deepEqual(state.translated, []);
});

test('a language change during a typeset waits its turn and ends in the last language', async () => {
    const holds = [];
    const mathJax = fakeMathJax({ typesetDelay: () => new Promise(resolve => holds.push(resolve)) });
    const { state, body, queue } = page({ mathJax });
    const settle = () => new Promise(resolve => setImmediate(resolve));
    const runs = [];
    state.language = 'zh-TW';
    runs.push(queue());
    await settle();
    // The zh-TW typeset is still running when the player picks another language, then zh-TW again.
    state.language = OTHER;
    runs.push(queue());
    holds.shift()();
    await settle();
    assert.equal(body.innerHTML, `[${OTHER}]${SOURCE}`, 'the turn for the other language puts the source back in that language');
    state.language = 'zh-TW';
    runs.push(queue());
    holds.shift()();
    await settle();
    holds.shift()();
    await Promise.all(runs);
    assert.equal(mathJax.mostAtOnce(), 1, 'only one typeset runs at a time');
    assert.deepEqual(mathJax.calls.filter(([kind]) => kind === 'typeset').map(([, html]) => html),
        [`[zh-TW]${SOURCE}`, `[${OTHER}]${SOURCE}`, `[zh-TW]${SOURCE}`]);
    assert.equal(body.innerHTML, `[zh-TW]${SOURCE}`);
});

test('changes queued before a turn runs end in the last language with one typeset', async () => {
    const mathJax = fakeMathJax();
    const { state, body, queue } = page({ mathJax });
    const runs = [];
    for (const language of ['zh-TW', OTHER, 'zh-TW']) {
        state.language = language;
        runs.push(queue());
    }
    await Promise.all(runs);
    assert.equal(body.innerHTML, `[zh-TW]${SOURCE}`);
    assert.equal(mathJax.calls.filter(([kind]) => kind === 'typeset').length, 1);
});

test('the same language twice does no extra work', async () => {
    const mathJax = fakeMathJax();
    const { queue, state } = page({ language: 'zh-TW', mathJax });
    await queue();
    await queue();
    assert.equal(mathJax.calls.length, 2);
    assert.deepEqual(state.translated, ['zh-TW']);
});

test('a typeset failure does not stop later language changes', async () => {
    const mathJax = fakeMathJax();
    mathJax.typesetPromise = async () => { throw new Error('typeset failed'); };
    const { state, body, queue } = page({ language: 'zh-TW', mathJax });
    const warn = console.warn;
    console.warn = () => {};
    try {
        await queue();
        state.language = 'en';
        await queue();
    } finally {
        console.warn = warn;
    }
    assert.equal(body.innerHTML, SOURCE);
});

test('without a math window or its source, nothing happens', async () => {
    const queue = createMathRetypeset({
        body: null, source: SOURCE, initialLanguage: 'en',
        getLanguage: () => 'zh-TW', getMathJax: () => null, translateTree: () => assert.fail('no translation'),
    });
    await queue();
    const body = { innerHTML: TYPESET_ENGLISH };
    const noSource = createMathRetypeset({
        body, source: undefined, initialLanguage: 'en',
        getLanguage: () => 'zh-TW', getMathJax: () => null, translateTree: () => assert.fail('no translation'),
    });
    await noSource();
    assert.equal(body.innerHTML, TYPESET_ENGLISH);
});
