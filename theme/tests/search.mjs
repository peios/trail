// Exercise the asynchronous search boundary with controllable Pagefind results.
import assert from "node:assert/strict";
import fs from "node:fs";
import vm from "node:vm";

import { makeDom } from "./dom.mjs";

const original = fs.readFileSync(new URL("../search.js", import.meta.url), "utf8");
const source = original.replace('import(site().base + "/pagefind/pagefind.js")', "loadPagefind()");
assert.notEqual(source, original, "the test must replace the external module boundary");

function run(loadPagefind) {
    const dom = makeDom();
    const context = {
        document: dom.document,
        location: dom.location,
        navigator: { platform: "Linux" },
        addEventListener() {},
        loadPagefind,
    };
    context.window = context;
    vm.runInNewContext(source, context);
    return {
        type(text) {
            dom.modal._input.value = text;
            return dom.modal._input.fire("input");
        },
        titles: () =>
            dom.modal._list.children.map((item) => item.children[0].children[0].textContent),
        status: () => dom.modal._status.textContent,
    };
}

function deferred() {
    let resolve;
    const promise = new Promise((done) => {
        resolve = done;
    });
    return { promise, resolve };
}

const result = (title) => ({
    results: [{ data: async () => ({ meta: { title }, url: "/result/" }) }],
});

let attempts = 0;
let t = run(async () => {
    if (++attempts === 1) {
        throw new Error("network failure");
    }
    return { init: async () => {}, debouncedSearch: async () => result("Recovered") };
});
await t.type("first");
assert.equal(t.status(), "Search is unavailable. Try again.");
await t.type("second");
assert.deepEqual(t.titles(), ["Recovered"]);

attempts = 0;
t = run(async () => ({
    init: async () => {
        if (++attempts === 1) {
            throw new Error("bad index");
        }
    },
    debouncedSearch: async () => result("Initialized"),
}));
await t.type("first");
assert.equal(t.status(), "Search is unavailable. Try again.");
await t.type("second");
assert.deepEqual(t.titles(), ["Initialized"]);

for (const replacement of ["", "/theme"]) {
    const old = deferred();
    t = run(async () => ({ init: async () => {}, debouncedSearch: () => old.promise }));
    const pending = t.type("old");
    await t.type(replacement);
    old.resolve(result("Stale"));
    await pending;
    assert.deepEqual(t.titles(), replacement ? ["/theme [system|light|dark]"] : []);
}

const old = deferred();
t = run(async () => ({
    init: async () => {},
    debouncedSearch: async (query) => (query === "old" ? old.promise : result("Newest")),
}));
const pending = t.type("old");
await t.type("new");
old.resolve(result("Stale"));
await pending;
assert.deepEqual(t.titles(), ["Newest"]);

for (const fail of ["search", "result"]) {
    t = run(async () => ({
        init: async () => {},
        debouncedSearch: async () => {
            if (fail === "search") {
                throw new Error("search failure");
            }
            return {
                results: [
                    {
                        data: async () => {
                            throw new Error("chunk failure");
                        },
                    },
                ],
            };
        },
    }));
    await t.type("query");
    assert.equal(t.status(), "Search is unavailable. Try again.");
    assert.deepEqual(t.titles(), []);
}
console.log("search recovery and stale-result tests passed");
