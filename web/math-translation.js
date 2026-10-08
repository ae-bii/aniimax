// The math window holds TeX that MathJax replaces with typeset nodes when the page loads. A
// catalog entry matches the untypeset text, so a typeset paragraph no longer matches it. For a
// language in RETYPESET_LANGUAGES, this module puts the English source back, translates it and
// then typesets it again. A language change away from one of them does the same, so no text of
// that language stays behind. Other languages keep the page as MathJax first typeset it.
import { languageReady, translateTree } from './i18n.js';

const RETYPESET_LANGUAGES = new Set(['zh-TW']);

// index.html keeps the untypeset English source while the page is parsed. A cached MathJax can
// typeset the page before this module runs, so the source cannot be read here.
const mathBody = document.querySelector('#mathModal .modal-body');
const mathSource = window.aniimaxMathSource;
let shownLanguage = document.documentElement.lang;

async function retypesetMath() {
    // MathJax typesets the page once after it loads. Swapping the text while it does would let
    // its output for the old text land in the new one, so wait for that typeset to end.
    await window.MathJax?.startup?.promise?.catch(() => {});
    const language = document.documentElement.lang;
    const previous = shownLanguage;
    if (!mathBody || !mathSource || language === previous) return;
    shownLanguage = language;
    if (!RETYPESET_LANGUAGES.has(language) && !RETYPESET_LANGUAGES.has(previous)) return;
    const mathJax = window.MathJax;
    mathJax?.typesetClear?.([mathBody]);
    mathBody.innerHTML = mathSource;
    translateTree(mathBody);
    // Before MathJax loads, it has no typesetPromise. Its own first typeset then sees this text.
    try {
        await mathJax?.typesetPromise?.([mathBody]);
    } catch (error) {
        console.warn('Could not typeset the math window:', error);
    }
}

// Run one retypeset at a time, in the order of the language changes.
let pending = Promise.resolve();
function queueRetypeset() {
    pending = pending.then(retypesetMath);
}

languageReady.then(queueRetypeset);
document.addEventListener('aniimax-language-change', queueRetypeset);
