// Connects the math window retypeset (see math-retypeset.js) to the page and its language changes.
import { languageReady, translateTree } from './i18n.js';
import { createMathRetypeset } from './math-retypeset.js';

const queueRetypeset = createMathRetypeset({
    body: document.querySelector('#mathModal .modal-body'),
    // index.html keeps the untypeset English source while the page is parsed. A cached MathJax can
    // typeset the page before this module runs, so the source cannot be read here.
    source: window.aniimaxMathSource,
    initialLanguage: document.documentElement.lang,
    getLanguage: () => document.documentElement.lang,
    getMathJax: () => window.MathJax,
    translateTree,
});

languageReady.then(queueRetypeset);
document.addEventListener('aniimax-language-change', queueRetypeset);
