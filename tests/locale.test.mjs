import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { runInNewContext } from 'node:vm';

const english = JSON.parse(await readFile(new URL('../web/locales/en.json', import.meta.url)));
const russian = JSON.parse(await readFile(new URL('../web/locales/ru.json', import.meta.url)));
const french = JSON.parse(await readFile(new URL('../web/locales/fr.json', import.meta.url)));

for (const catalog of [russian, french]) {
    assert.deepEqual(new Set(english), new Set(Object.keys(catalog)));
    for (const [source, target] of Object.entries(catalog)) {
        const slots = text => new Set(text.match(/\{[a-z_]+\}/gi) || []);
        assert.deepEqual(slots(source), slots(target), source);
    }
}

const saved = new Map([['aniimax-language', 'ru']]);
globalThis.localStorage = { getItem: key => saved.get(key), setItem: (key, value) => saved.set(key, value) };
let releaseCatalog;
globalThis.fetch = async url => {
    await new Promise(resolve => { releaseCatalog = resolve; });
    return { ok: true, json: async () => JSON.parse(await readFile(url)) };
};
globalThis.Node = { TEXT_NODE: 3, ELEMENT_NODE: 1 };
globalThis.NodeFilter = { SHOW_ELEMENT: 1, SHOW_TEXT: 4 };
const node = { nodeType: 3, nodeValue: 'Your Homeland' };
const labelled = {
    nodeType: 1, values: new Map([['data-label', 'Profit']]),
    hasAttribute(name) { return this.values.has(name); },
    getAttribute(name) { return this.values.get(name); },
    setAttribute(name, value) { this.values.set(name, value); },
};
const body = { nodeType: 1, hasAttribute: () => false };
let change;
const selector = { value: 'en', add() {}, addEventListener: (_, listener) => { change = listener; } };
globalThis.Option = class { constructor(label, value) { this.label = label; this.value = value; } };
globalThis.document = {
    body, documentElement: { lang: 'en' }, title: '', getElementById: () => selector,
    dispatchEvent() {},
    createTreeWalker: () => ({ currentNode: null, index: 0, nextNode() {
        this.currentNode = [node, labelled][this.index++];
        return !!this.currentNode;
    } }),
};
globalThis.MutationObserver = class { observe() {} };

const { translate, languageReady } = await import('../web/i18n.js');
assert.equal(node.nodeValue, 'Your Homeland');
assert.equal(document.documentElement.lang, 'en');
assert.equal(selector.value, 'ru');
assert.equal(saved.get('aniimax-language'), 'ru');


const appSource = await readFile(new URL('../web/app.js', import.meta.url), 'utf8');
const handlerStart = appSource.indexOf("document.addEventListener('aniimax-language-change'");
const handlerEnd = appSource.indexOf('async function loadRecipeIndex()', handlerStart);
assert.ok(handlerStart >= 0 && handlerEnd > handlerStart);
const handlerSource = appSource.slice(handlerStart, handlerEnd);
const englishRecipe = 'Wheat (Farmland)';
const russianRecipe = 'Пшеница (Пахотная земля)';
for (const [before, after, typed] of [
    [englishRecipe, russianRecipe, englishRecipe],
    [russianRecipe, englishRecipe, russianRecipe],
    [englishRecipe, russianRecipe, 'unfinished recipe'],
]) {
    const input = { value: typed, validity: 'previous error', setCustomValidity(value) { this.validity = value; } };
    const options = { options: [{ value: before }] };
    let languageChanged;
    runInNewContext(handlerSource, {
        document: {
            addEventListener: (_, listener) => { languageChanged = listener; },
            getElementById: id => id === 'skip-input' ? input : options,
        },
        renderRecipeOptions: () => { options.options = [{ value: after }]; },
    });
    languageChanged();
    assert.equal(input.value, typed === before ? after : typed);
    assert.equal(input.validity, typed === before ? '' : 'previous error');
}
releaseCatalog();
await languageReady;
assert.equal(node.nodeValue, 'Ваша Родина');
assert.equal(document.documentElement.lang, 'ru');
assert.equal(labelled.getAttribute('data-label'), 'Прибыль');
assert.equal(translate('Used for Wheat; the rest sells directly'), 'Используется для Пшеница; остаток продаётся напрямую');
assert.equal(translate('in 2h 4m'), 'через 2ч 4м');
assert.equal(translate('20 (everything unlocked)'), '20 (всё открыто)');
assert.equal(translate('2 on'), '2 активных');
assert.equal(translate('3 skipped'), '3 исключённых');
assert.equal(translate('+ Add level'), '+ Добавить уровень');
assert.equal(translate('Start from the Best team'), 'Начать с лучшей команды');
assert.equal(translate('Faithful or Tenacious'), 'Верный или Упорный');
assert.equal(translate('unmapped term'), 'unmapped term');
assert.equal(translate('Unknown status'), 'Unknown status');
assert.equal(translate('Home Coins/min'), 'Монеты дома/мин');
assert.equal(translate('Plot 1'), 'Участок 1');
assert.equal(translate('RV 13'), 'Автодом 13');
assert.equal(translate('Farmland, Wheat, 20 trips/hour · 2.0 tiles from storage'), 'Пахотная земля, Пшеница, 20 рейсов/ч · расстояние до склада: 2.0 клет.');
assert.equal(translate('Storage Unit, Where everything is carried, 20 trips/hour'), 'Склад, сюда доставляют всё произведённое, 20 рейсов/ч');
assert.equal(translate('Fire: Cooking, smelting and heat'), 'Огонь: Готовка, плавка и нагрев');
assert.equal(translate('Fire level'), 'Уровень способности Огонь');
assert.equal(translate('Grass Lv.1'), 'Трава ур. 1');
assert.equal(translate('Flowers in a Bottle'), 'Цветы в бутылке');
assert.equal(translate('nothing to check · 1 ms'), 'нечего проверять · 1 мс');
assert.equal(translate('40 ms'), '40 мс');
assert.equal(translate(' · 19 ms'), ' · 19 мс');
assert.equal(translate(' · 1.2 s'), ' · 1.2 с');
assert.equal(translate('1.2 s'), '1.2 с');
assert.equal(translate('No level-4 Perfumery Aniimo is known in the game yet'), 'Анимо способности «Парфюмерия» уровня 4 пока не встречались в игре');
assert.equal(translate('No level-4 Perfumery Aniimo is known in the game yet. Plan as though you have one?'), 'Анимо способности «Парфюмерия» уровня 4 пока не встречались в игре. Считать, что такой Анимо у вас есть?');
assert.equal(translate('Grass Lv.1 · Sowing farmland'), 'Трава ур. 1 · Посев на пашне');
assert.equal(translate('Hauling, any level · carries produce to storage; add more if produce piles up'), 'Перенос, любой уровень · доставляет продукцию на склад; добавьте больше Анимо, если она скапливается');
assert.equal(translate('Instinctive or Energetic'), 'Инстинктивный или Энергичный');
selector.value = 'en';
await change();
assert.equal(node.nodeValue, 'Your Homeland');
assert.equal(labelled.getAttribute('data-label'), 'Profit');
selector.value = 'ru';
await change();
assert.equal(node.nodeValue, 'Ваша Родина');
assert.equal(labelled.getAttribute('data-label'), 'Прибыль');
assert.equal(saved.get('aniimax-language'), 'ru');

selector.value = 'fr';
const frenchChange = change();
releaseCatalog();
await frenchChange;

assert.equal(node.nodeValue, 'Votre Logie');
assert.equal(document.documentElement.lang, 'fr');
assert.equal(labelled.getAttribute('data-label'), 'Profit');
assert.equal(saved.get('aniimax-language'), 'fr');
