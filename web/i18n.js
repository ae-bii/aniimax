// Register a language here and add its JSON catalog; English phrases are the source keys.
const languages = { en: 'English', ru: 'Русский', 'zh-TW': '繁體中文' };
const locales = { en: 'en-US', ru: 'ru-RU', 'zh-TW': 'zh-TW' };
// How a language matches templates and splits a phrase that has no catalog entry.
// - separators: split the phrase at these, in this order, and translate each part.
// - joiners: the text that joins the translated parts. A missing separator joins with itself.
// - rankByFixedText: try a template with more fixed text first, not a longer one.
// - strictSlots: a text slot holds whole brackets and no slot break.
const defaultRules = { separators: ['; ', '. '], joiners: { '. ': ' ' }, rankByFixedText: false, strictSlots: false };
const languageRules = {
    'zh-TW': {
        separators: ['\n', ' · ', '; ', '. ', ', '],
        joiners: { '. ': '', '; ': '；', ', ': '、' },
        rankByFixedText: true,
        strictSlots: true,
    },
};
const SLOT = /\{[a-z_]+\}/gi;
const NUMBER_SLOT = /^\{(?:days|hours|minutes|seconds|n|level|trips|distance|plots)\}$/;
// With strictSlots, a text slot never holds one of these. The phrase is split at them instead.
const SLOT_BREAKS = ['\n', ' · ', '; '];

function rulesFor(code) {
    return languageRules[code] || defaultRules;
}
const catalogs = { en: {} };
const templates = {};
let language = 'en';
let preferredLanguage = 'en';
try { preferredLanguage = localStorage.getItem('aniimax-language') || 'en'; } catch (_) { /* Private browsing can disable storage. */ }
if (!Object.hasOwn(languages, preferredLanguage)) preferredLanguage = 'en';

async function load(code) {
    if (!catalogs[code]) {
        const response = await fetch(new URL(`./locales/${code}.json`, import.meta.url));
        if (!response.ok) throw new Error(`Could not load ${code} translations`);
        catalogs[code] = await response.json();
        templates[code] = Object.entries(catalogs[code])
            .filter(([source, target]) => /\{[a-z_]+\}/i.test(source) && /\{[a-z_]+\}/i.test(target))
            // A template with more fixed text is more specific, so rankByFixedText tries it first.
            .sort(([left], [right]) => (rulesFor(code).rankByFixedText ? literalLength(right) - literalLength(left) : 0)
                || right.length - left.length)
            .map(([source, target]) => {
                const slots = [...source.matchAll(/\{[a-z_]+\}/gi)].map(match => match[0]);
                const parts = source.split(/\{[a-z_]+\}/gi);
                const pattern = parts.map((part, i) => escapeRegExp(part) + (i < slots.length
                    ? (NUMBER_SLOT.test(slots[i]) ? '([0-9][0-9.,\\s]*)' : '(.+?)')
                    : '')).join('');
                const textSlots = slots.map(slot => !NUMBER_SLOT.test(slot));
                // A separator in the fixed text, as in "{ability} Lv.{level} · {detail}", lets its slot hold one.
                const breaks = SLOT_BREAKS.filter(separator => !source.includes(separator));
                return { pattern: new RegExp(`^${pattern}$`), slots, textSlots, breaks, target };
            });
    }
}

const textSources = new WeakMap();
const attributeSources = new WeakMap();
const attributes = ['title', 'aria-label', 'aria-description', 'placeholder', 'data-label', 'data-tooltip', 'data-tip', 'data-tip-detail', 'data-tip-stats', 'data-tip-text'];

export function translate(value) {
    if (language === 'en' || !value) return value;
    const phrases = catalogs[language];
    const trimmed = value.trim();
    const translated = phrases[trimmed];
    if (translated) return value.replace(trimmed, translated);
    // Dynamic labels keep their values while the source sentence remains translatable.
    const filled = fillTemplate(trimmed);
    if (filled) return value.replace(trimmed, filled);
    for (const separator of rulesFor(language).separators) {
        const localized = translateParts(trimmed, separator);
        if (localized) return value.replace(trimmed, localized);
    }
    return value;
}

// Fills the first template that matches. Returns null when none matches.
function fillTemplate(trimmed) {
    for (const template of templates[language] || []) {
        const match = trimmed.match(template.pattern);
        if (!match) continue;
        if (rulesFor(language).strictSlots && !template.textSlots.every((text, i) => !text || fits(match[i + 1], template.breaks))) continue;
        let result = template.target;
        template.slots.forEach((slot, i) => {
            const captured = match[i + 1];
            const whole = captured === trimmed ? captured : translate(captured);
            const value = whole === captured ? captured.split(', ').map(part => part === trimmed ? part : translate(part)).join(', ') : whole;
            result = result.replace(slot, value);
        });
        return result;
    }
    return null;
}

// A text slot holds whole brackets and no separator, so "A (x), B (y)" is not "{name} ({detail})".
function fits(text, breaks) {
    return balanced(text) && !breaks.some(separator => text.includes(separator));
}

function balanced(text) {
    let depth = 0;
    for (const char of text) {
        if (char === '(') depth += 1;
        else if (char === ')' && --depth < 0) return false;
    }
    return depth === 0;
}

// Translates each part of `text` between separators. Returns null when no part changes.
function translateParts(text, separator) {
    if (!text.includes(separator)) return null;
    const parts = text.split(separator);
    // A sentence keeps its full stop, so the catalog entry for it still matches.
    const sources = separator === '. ' ? parts.map((part, i) => part + (i < parts.length - 1 ? '.' : '')) : parts;
    const localized = sources.map(part => translate(part));
    if (localized.every((part, i) => part === sources[i])) return null;
    const { joiners } = rulesFor(language);
    const joiner = joiners[separator] ?? defaultRules.joiners[separator] ?? separator;
    return localized.join(joiner);
}

function literalLength(source) {
    return source.replace(SLOT, '').length;
}

export function currentLocale() {
    return locales[language];
}

function escapeRegExp(text) {
    return text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

function updateText(node) {
    const previous = textSources.get(node);
    if (previous?.shown === node.nodeValue) {
        if (language === previous.language) return;
    }
    const source = previous?.shown === node.nodeValue ? previous.source : node.nodeValue;
    const shown = translate(source);
    textSources.set(node, { source, shown, language });
    if (shown !== node.nodeValue) node.nodeValue = shown;
}

function updateAttributes(element) {
    let sources = attributeSources.get(element);
    if (!sources) { sources = new Map(); attributeSources.set(element, sources); }
    for (const name of attributes) {
        if (!element.hasAttribute(name)) continue;
        const current = element.getAttribute(name);
        const previous = sources.get(name);
        if (previous?.shown === current && previous.language === language) continue;
        const source = previous?.shown === current ? previous.source : current;
        const shown = translate(source);
        sources.set(name, { source, shown, language });
        if (shown !== current) element.setAttribute(name, shown);
    }
}

export function sourceAttribute(element, name) {
    const current = element.getAttribute(name);
    const previous = attributeSources.get(element)?.get(name);
    return previous?.shown === current ? previous.source : current;
}

// Translates `root` and everything in it now, without waiting for the MutationObserver.
export function translateTree(root) {
    update(root);
}

function update(root) {
    if (root.nodeType === Node.TEXT_NODE) return updateText(root);
    if (root.nodeType !== Node.ELEMENT_NODE) return;
    updateAttributes(root);
    const walker = document.createTreeWalker(root, NodeFilter.SHOW_ELEMENT | NodeFilter.SHOW_TEXT);
    while (walker.nextNode()) {
        if (walker.currentNode.nodeType === Node.TEXT_NODE) updateText(walker.currentNode);
        else updateAttributes(walker.currentNode);
    }
}

const selector = document.getElementById('language-switch');
for (const [code, name] of Object.entries(languages)) selector.add(new Option(name, code));
selector.value = preferredLanguage;
document.documentElement.lang = language;
document.title = translate('Aniimax - Aniimo Production Optimizer');
update(document.body);
let languageRequest = 0;
async function selectLanguage(code) {
    const request = ++languageRequest;
    try { await load(code); } catch (error) {
        console.warn(error);
        if (request === languageRequest) selector.value = language;
        return;
    }
    if (request !== languageRequest) return;
    language = code;
    try { localStorage.setItem('aniimax-language', language); } catch (_) { /* Language still changes for this tab. */ }
    document.documentElement.lang = language;
    document.title = translate('Aniimax - Aniimo Production Optimizer');
    update(document.body);
    document.dispatchEvent(new Event('aniimax-language-change'));
}
selector.addEventListener('change', () => selectLanguage(selector.value));

new MutationObserver(records => {
    for (const record of records) {
        if (record.type === 'characterData') updateText(record.target);
        else if (record.type === 'attributes') updateAttributes(record.target);
        else record.addedNodes.forEach(update);
    }
}).observe(document.body, { subtree: true, childList: true, characterData: true, attributes: true, attributeFilter: attributes });

export const languageReady = selectLanguage(preferredLanguage);
