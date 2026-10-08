// Tests for the Traditional Chinese (zh-TW) web text. They check only zh-TW.
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';

// A small page for i18n.js. It starts in zh-TW.
const saved = new Map([['aniimax-language', 'zh-TW']]);
globalThis.localStorage = { getItem: key => saved.get(key), setItem: (key, value) => saved.set(key, value) };
globalThis.fetch = async url => ({ ok: true, json: async () => JSON.parse(await readFile(url)) });
globalThis.Node = { TEXT_NODE: 3, ELEMENT_NODE: 1 };
globalThis.NodeFilter = { SHOW_ELEMENT: 1, SHOW_TEXT: 4 };
const node = { nodeType: 3, nodeValue: 'Your Homeland' };
const selector = { value: 'en', add() {}, addEventListener() {} };
globalThis.Option = class { constructor(label, value) { this.label = label; this.value = value; } };
globalThis.document = {
    body: { nodeType: 1, hasAttribute: () => false }, documentElement: { lang: 'en' }, title: '',
    getElementById: () => selector, dispatchEvent() {},
    createTreeWalker: () => ({ currentNode: null, done: false, nextNode() {
        this.currentNode = this.done ? null : node;
        this.done = true;
        return !!this.currentNode;
    } }),
};
globalThis.MutationObserver = class { observe() {} };

const i18n = await import('../web/i18n.js');

const catalog = JSON.parse(await readFile(new URL('../web/locales/zh-TW.json', import.meta.url)));
const translate = i18n.createZhTwTranslator(catalog);

// Each entry keeps the placeholders of its English phrase.
for (const [source, target] of Object.entries(catalog)) {
    const slots = text => new Set(text.match(/\{[a-z_]+\}/gi) || []);
    assert.deepEqual(slots(source), slots(target), source);
}

// English text that the catalog does not hold, for example a phrase added later, stays in English.
assert.equal(translate('A phrase added after the zh-TW catalog'), 'A phrase added after the zh-TW catalog');
assert.equal(translate('unmapped term'), 'unmapped term');
assert.equal(translate('unmapped term, Wheat'), 'unmapped term、小麥');
assert.equal(translate('constructor'), 'constructor');

assert.equal(translate('  Your Homeland '), '  你的家園 ');
assert.equal(translate('in 2h 4m'), '2 小時 4 分後');
assert.equal(translate('Used for Wheat; the rest sells directly'), '用於小麥；其餘直接販售');
assert.equal(translate('Target Moonray Wheat'), '目標：月芒穗');
assert.equal(translate('RV 13'), '露營車家園 13');
assert.equal(translate(' · 19 ms'), ' · 19 毫秒');
assert.equal(translate('Grass Lv.1 · Sowing farmland'), '草 Lv.1 · 田地播種');
// Chinese sentences join with no space between them.
assert.equal(translate('No level-4 Perfumery Aniimo is known in the game yet. Plan as though you have one?'), '遊戲中目前還沒有已知的 4 級調香伊莫。要當作你擁有一隻來規劃嗎？');
// Phrases built at runtime: lists, tooltips with ' · ' and lines, and amounts with items.
assert.equal(translate('9,187,977 Home Coins, 455 Sintered Ore Brick'), '9,187,977 家園幣、455 燒結礦磚');
assert.equal(translate('Fire Lv.4 · Practical (S) (+20% speed) · Chimney Kiln (Coarse-Sifted Ore), Chimney Kiln (Sintered Ore Brick)'),
    '火 Lv.4 · 性格S（速度 +20%） · 煙囪鍛燒爐（粗篩礦料）、煙囪鍛燒爐（燒結礦磚）');
assert.equal(translate('Woodland: Chestnut ×9'), '林地：栗子 ×9');
assert.equal(translate('+17% Home Coins (+26,044/hour)'), '家園幣 +17%（+26,044/小時）');
assert.equal(translate('Lv.1: Sea Salt\nLv.3: Pearl (needs Warm)'), 'Lv.1：海鹽\nLv.3：珍珠（需要溫暖）');
assert.equal(translate('29 Aniimo for this plan; an RV level 13 homeland holds 34'), '此規劃需要 29 隻伊莫；露營車等級 13 的家園可容納 34 隻');
// Opportunities: ability Aniimo, the player's own Aniimo, facilities and the status line.
assert.equal(translate('Fire Aniimo Lv.2–4'), '火伊莫 Lv.2–4');
assert.equal(translate('One Fire 3, Water 2 Aniimo: Water Lv.4'), '其中一隻 火 3、水 2 伊莫：水 Lv.4');
assert.equal(translate('1 Blazing Stove to Lv.4'), '1 座超旺爐灶升至 Lv.4');
assert.equal(translate('~−2h 4m level-up (30h 5m)'), '~升級 −2 小時 4 分（30 小時 5 分）');
assert.equal(translate('Ranked by level-up time, then Home Coins. 7 of 14 help. Within RV 4 limits.'),
    '依升級時間，其次是家園幣排序。已檢查 14 項，其中 7 項有幫助。在露營車家園 4 的限制內。');
// A text slot never holds part of two bracketed names.
assert.equal(translate('Blazing Stove (Ginseng Chestnut Cake), Simmering Pot (Grape Jam)'), '超旺爐灶（人參栗子糕）、熬煮鍋（葡萄醬）');

// i18n.js switches to zh-TW, uses the zh-TW rules and gives the zh-TW number format.
await i18n.languageReady;
assert.equal(selector.value, 'zh-TW');
assert.equal(document.documentElement.lang, 'zh-TW');
assert.equal(i18n.currentLocale(), 'zh-TW');
assert.equal(node.nodeValue, '你的家園');
assert.equal(i18n.translate('unmapped term, Wheat'), 'unmapped term、小麥');
