// Register a language here and add its JSON catalog; English phrases are the source keys.
const languages = { en: 'English', ru: 'Русский' };
const catalogs = { en: {} };
const templates = {};
let language = 'en';
try { language = localStorage.getItem('aniimax-language') || 'en'; } catch (_) { /* Private browsing can disable storage. */ }
if (!Object.hasOwn(languages, language)) language = 'en';

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

try { await load(language); } catch (error) {
    console.warn(error);
    language = 'en';
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

export function currentLocale() {
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
selector.value = language;
document.documentElement.lang = language;
document.title = translate('Aniimax - Aniimo Production Optimizer');
update(document.body);
selector.addEventListener('change', async () => {
    try { await load(selector.value); } catch (error) {
        console.warn(error);
        selector.value = language;
        return;
    }
    language = selector.value;
    try { localStorage.setItem('aniimax-language', language); } catch (_) { /* Language still changes for this tab. */ }
    document.documentElement.lang = language;
    document.title = translate('Aniimax - Aniimo Production Optimizer');
    update(document.body);
    document.dispatchEvent(new Event('aniimax-language-change'));
});

new MutationObserver(records => {
    for (const record of records) {
        if (record.type === 'characterData') updateText(record.target);
        else if (record.type === 'attributes') updateAttributes(record.target);
        else record.addedNodes.forEach(update);
    }
}).observe(document.body, { subtree: true, childList: true, characterData: true, attributes: true, attributeFilter: attributes });
