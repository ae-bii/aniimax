import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';

const french = JSON.parse(await readFile(new URL('../web/locales/fr.json', import.meta.url)));
const slots = text => new Set(text.match(/\{[a-z_]+\}/gi) || []);
for (const [source, target] of Object.entries(french)) {
    assert.deepEqual(slots(source), slots(target), source);
}
const phrases = {
    'River-Washed Stones': 'Pierres polies de rivière',
    'Premium River-Washed Stones': 'Pierres polies de rivière premium',
    '+ Add level': '+ Ajouter un niveau',
    'Best plan found in the time allowed; the best possible is at most {gap}% higher.':
        'Meilleur plan trouvé dans le temps imparti ; l\'optimum peut être jusqu\'à {gap} % supérieur.',
    "The exact planner couldn't run, so this plan comes from the backup planner and may not be the very best. Reloading the page usually fixes this.":
        "Le planificateur exact n'a pas pu s'exécuter ; ce plan provient donc du planificateur de secours et n'est peut-être pas optimal. Recharger la page résout généralement ce problème.",
    "The exact planner couldn't run {reason}, so this plan comes from the backup planner and may not be the very best. Reloading the page usually fixes this.":
        "Le planificateur exact n'a pas pu s'exécuter {reason} ; ce plan provient donc du planificateur de secours et n'est peut-être pas optimal. Recharger la page résout généralement ce problème.",
};
for (const [source, target] of Object.entries(phrases)) assert.equal(french[source], target);

const saved = new Map([['aniimax-language', 'en']]);
globalThis.localStorage = { getItem: key => saved.get(key), setItem: (key, value) => saved.set(key, value) };
globalThis.fetch = async url => ({ ok: true, json: async () => JSON.parse(await readFile(url)) });
globalThis.Node = { TEXT_NODE: 3, ELEMENT_NODE: 1 };
globalThis.NodeFilter = { SHOW_ELEMENT: 1, SHOW_TEXT: 4 };
globalThis.Option = class {};
globalThis.MutationObserver = class { observe() {} };
const text = { nodeType: 3, nodeValue: 'Your Homeland' };
const button = {
    nodeType: 1, values: new Map([['aria-label', '+ Add level']]),
    hasAttribute(name) { return this.values.has(name); },
    getAttribute(name) { return this.values.get(name); },
    setAttribute(name, value) { this.values.set(name, value); },
};
let change;
const selector = { value: 'en', add() {}, addEventListener: (_, listener) => { change = listener; } };
globalThis.document = {
    body: { nodeType: 1, hasAttribute: () => false }, documentElement: { lang: 'en' }, title: '',
    getElementById: () => selector, dispatchEvent() {},
    createTreeWalker: () => ({ index: 0, nextNode() {
        this.currentNode = [text, button][this.index++];
        return !!this.currentNode;
    } }),
};
const { translate, languageReady, currentLocale } = await import('../web/i18n.js');
await languageReady;
selector.value = 'fr';
await change();
assert.equal(text.nodeValue, 'Votre Logie');
assert.equal(button.getAttribute('aria-label'), '+ Ajouter un niveau');
assert.equal(document.documentElement.lang, 'fr');
assert.equal(currentLocale(), 'fr-FR');
assert.equal(saved.get('aniimax-language'), 'fr');
for (const [source, target] of Object.entries(phrases)) {
    const values = value => value.replace('{gap}', '12').replace('{reason}', '(test)');
    assert.equal(translate(values(source)), values(target), source);
}
selector.value = 'en';
await change();
assert.equal(text.nodeValue, 'Your Homeland');
assert.equal(button.getAttribute('aria-label'), '+ Add level');
assert.equal(currentLocale(), 'en-US');
assert.equal(saved.get('aniimax-language'), 'en');
console.log('French locale regression checks passed.');
