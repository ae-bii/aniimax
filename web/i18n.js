// Register a language here and add its JSON catalog; English phrases are the source keys.
const languages = { en: 'English', ru: 'Русский', 'zh-TW': '繁體中文' };
const catalogs = { en: {} };
const templates = {};
let language = 'en';
let translateZhTw;
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
            .sort(([left], [right]) => right.length - left.length)
            .map(([source, target]) => {
                const slots = [...source.matchAll(/\{[a-z_]+\}/gi)].map(match => match[0]);
                const parts = source.split(/\{[a-z_]+\}/gi);
                const pattern = parts.map((part, i) => escapeRegExp(part) + (i < slots.length
                    ? (/^\{(?:days|hours|minutes|seconds|n|level|trips|distance|plots)\}$/.test(slots[i])
                        ? '([0-9][0-9.,\\s]*)' : '(.+?)')
                    : '')).join('');
                return { pattern: new RegExp(`^${pattern}$`), slots, target };
            });
    }
}

const textSources = new WeakMap();
const attributeSources = new WeakMap();
const attributes = ['title', 'aria-label', 'aria-description', 'placeholder', 'data-label', 'data-tooltip', 'data-tip', 'data-tip-detail', 'data-tip-stats', 'data-tip-text'];

export function translate(value) {
    if (language === 'en' || !value) return value;
    if (language === 'zh-TW') return (translateZhTw ??= createZhTwTranslator(catalogs['zh-TW']))(value);
    const phrases = catalogs[language];
    const trimmed = value.trim();
    const translated = phrases[trimmed];
    if (translated) return value.replace(trimmed, translated);
    // Dynamic labels keep their values while the source sentence remains translatable.
    for (const { pattern, slots, target } of templates[language] || []) {
        const match = trimmed.match(pattern);
        if (!match) continue;
        let result = target;
        slots.forEach((slot, i) => {
            const captured = match[i + 1];
            const whole = captured === trimmed ? captured : translate(captured);
            const value = whole === captured ? captured.split(', ').map(part => part === trimmed ? part : translate(part)).join(', ') : whole;
            result = result.replace(slot, value);
        });
        return value.replace(trimmed, result);
    }
    if (trimmed.includes('; ')) {
        const parts = trimmed.split('; ');
        const localized = parts.map(part => translate(part));
        if (localized.some((part, i) => part !== parts[i])) return value.replace(trimmed, localized.join('; '));
    }
    if (trimmed.includes('. ')) {
        const parts = trimmed.split('. ');
        const localized = parts.map((part, i) => translate(part + (i < parts.length - 1 ? '.' : '')));
        if (localized.some((part, i) => part !== parts[i] + (i < parts.length - 1 ? '.' : ''))) {
            return value.replace(trimmed, localized.join(' '));
        }
    }
    return value;
}

// Traditional Chinese (zh-TW) rules. translate() uses them only for zh-TW, so the other languages keep
// the rules above. They differ from those rules:
// - An unknown phrase is split at line breaks, ' · ' and ', ' as well, and list items join with '、'.
// - A template with more fixed text is tried first.
// - A text slot holds whole brackets and no slot break, so "A (x), B (y)" is not "{name} ({detail})".
// Text that the zh-TW catalog does not hold, for example new English text, stays in English.
const ZH_TW_SLOT = /\{[a-z_]+\}/gi;
const ZH_TW_NUMBER_SLOT = /^\{(?:days|hours|minutes|seconds|n|level|trips|distance|plots)\}$/;
const ZH_TW_SEPARATORS = ['\n', ' · ', '; ', '. ', ', '];
const ZH_TW_JOINERS = { '. ': '', '; ': '；', ', ': '、' };
// A text slot never holds one of these, unless the fixed text of its template holds it.
const ZH_TW_SLOT_BREAKS = ['\n', ' · ', '; '];

/** Gives a function that translates English text with the zh-TW `catalog`. */
export function createZhTwTranslator(catalog) {
    const zhTemplates = zhTwBuildTemplates(catalog);

    function translateText(value) {
        if (!value) return value;
        const trimmed = value.trim();
        const translated = Object.hasOwn(catalog, trimmed) ? catalog[trimmed] : '';
        const result = translated || fillTemplate(trimmed) || translateParts(trimmed);
        return result ? value.replace(trimmed, () => result) : value;
    }

    // Fills the first template that matches. Returns null when none matches.
    function fillTemplate(trimmed) {
        for (const template of zhTemplates) {
            const match = trimmed.match(template.pattern);
            if (!match) continue;
            if (!template.textSlots.every((text, i) => !text || zhTwFits(match[i + 1], template.breaks))) continue;
            let result = template.target;
            template.slots.forEach((slot, i) => {
                const captured = match[i + 1];
                const whole = captured === trimmed ? captured : translateText(captured);
                const value = whole === captured ? captured.split(', ').map(part => part === trimmed ? part : translateText(part)).join(', ') : whole;
                result = result.replace(slot, () => value);
            });
            return result;
        }
        return null;
    }

    // Translates each part between the first separator that changes a part. Returns null when none does.
    function translateParts(text) {
        for (const separator of ZH_TW_SEPARATORS) {
            if (!text.includes(separator)) continue;
            const parts = text.split(separator);
            // A sentence keeps its full stop, so the catalog entry for it still matches.
            const sources = separator === '. ' ? parts.map((part, i) => part + (i < parts.length - 1 ? '.' : '')) : parts;
            const localized = sources.map(part => translateText(part));
            if (localized.every((part, i) => part === sources[i])) continue;
            return localized.join(ZH_TW_JOINERS[separator] ?? separator);
        }
        return null;
    }

    return translateText;
}

function zhTwBuildTemplates(catalog) {
    return Object.entries(catalog)
        .filter(([source, target]) => /\{[a-z_]+\}/i.test(source) && /\{[a-z_]+\}/i.test(target))
        // A template with more fixed text is more specific, so it comes first.
        .sort(([left], [right]) => zhTwLiteralLength(right) - zhTwLiteralLength(left) || right.length - left.length)
        .map(([source, target]) => {
            const slots = [...source.matchAll(ZH_TW_SLOT)].map(match => match[0]);
            const parts = source.split(ZH_TW_SLOT);
            const pattern = parts.map((part, i) => escapeRegExp(part) + (i < slots.length
                ? (ZH_TW_NUMBER_SLOT.test(slots[i]) ? '([0-9][0-9.,\\s]*)' : '(.+?)')
                : '')).join('');
            const textSlots = slots.map(slot => !ZH_TW_NUMBER_SLOT.test(slot));
            // A separator in the fixed text, as in "{ability} Lv.{level} · {detail}", lets its slot hold one.
            const breaks = ZH_TW_SLOT_BREAKS.filter(separator => !source.includes(separator));
            return { pattern: new RegExp(`^${pattern}$`), slots, textSlots, breaks, target };
        });
}

// A text slot holds whole brackets and no slot break.
function zhTwFits(text, breaks) {
    return zhTwBalanced(text) && !breaks.some(separator => text.includes(separator));
}

function zhTwBalanced(text) {
    let depth = 0;
    for (const char of text) {
        if (char === '(') depth += 1;
        else if (char === ')' && --depth < 0) return false;
    }
    return depth === 0;
}

function zhTwLiteralLength(source) {
    return source.replace(ZH_TW_SLOT, '').length;
}

export function currentLocale() {
    if (language === 'zh-TW') return 'zh-TW';
    return language === 'ru' ? 'ru-RU' : 'en-US';
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
