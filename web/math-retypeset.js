// The math window holds TeX that MathJax replaces with typeset nodes when the page loads. A
// catalog entry matches the untypeset text, so a typeset paragraph no longer matches it. For a
// language in `languages`, the retypeset puts the English source back, translates it and then
// typesets it again. A change away from one of those languages does the same, so no text of that
// language stays behind. Other languages keep the window as MathJax first typeset it.
//
// This module touches no page globals. math-translation.js connects it to the page, and the tests
// give it stand-ins.

export const RETYPESET_LANGUAGES = new Set(['zh-TW']);

// Returns a function that queues one retypeset for the current language. The queued retypesets
// run one at a time, in order. Each call returns a promise that settles when its turn is done.
export function createMathRetypeset({
    body,
    source,
    initialLanguage,
    getLanguage,
    getMathJax,
    translateTree,
    languages = RETYPESET_LANGUAGES,
}) {
    let shownLanguage = initialLanguage;

    async function retypeset() {
        // MathJax typesets the page once after it loads. Swapping the text while it does would let
        // its output for the old text land in the new one, so wait for that typeset to end.
        await getMathJax()?.startup?.promise?.catch(() => {});
        const language = getLanguage();
        const previous = shownLanguage;
        if (!body || !source || language === previous) return;
        shownLanguage = language;
        if (!languages.has(language) && !languages.has(previous)) return;
        const mathJax = getMathJax();
        mathJax?.typesetClear?.([body]);
        body.innerHTML = source;
        translateTree(body);
        // Before MathJax loads, it has no typesetPromise. Its own first typeset then sees this text.
        try {
            await mathJax?.typesetPromise?.([body]);
        } catch (error) {
            console.warn('Could not typeset the math window:', error);
        }
    }

    let pending = Promise.resolve();
    return function queueRetypeset() {
        // A failed turn must not stop the turns after it.
        pending = pending.then(retypeset).catch(error => console.warn('Could not retranslate the math window:', error));
        return pending;
    };
}
