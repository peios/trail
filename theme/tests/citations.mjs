import assert from "node:assert/strict";
import fs from "node:fs";
import vm from "node:vm";

const template = fs.readFileSync(new URL("../base.html", import.meta.url), "utf8");
const script = [...template.matchAll(/<script>([\s\S]*?)<\/script>/g)]
    .map((match) => match[1])
    .find((source) => source.includes("window.trailToggleCitations"));
assert.ok(script);

for (const outcome of ["success", "failure", "unavailable"]) {
    let click;
    let copied;
    const classes = new Set();
    const anchor = {
        dataset: { citation: "Book *statement" },
        classList: { add: (name) => classes.add(name), remove: (name) => classes.delete(name) },
        getAttribute: () => "#statement",
    };
    const context = {
        document: {
            body: {},
            addEventListener: (name, handler) => {
                click = handler;
            },
        },
        localStorage: { getItem: () => null },
        history: { replaceState() {} },
        setTimeout() {},
        navigator:
            outcome === "unavailable"
                ? {}
                : {
                      clipboard: {
                          async writeText(text) {
                              if (outcome === "failure") {
                                  throw new Error("permission denied");
                              }
                              copied = text;
                          },
                      },
                  },
    };
    context.window = context;
    vm.runInNewContext(script, context);
    click({ target: { closest: () => anchor }, preventDefault() {} });
    await Promise.resolve();
    assert.equal(classes.has("copied"), outcome === "success");
    if (outcome === "success") {
        assert.equal(copied, "Book *statement");
    } else {
        assert.match(anchor.title, /Copy failed.*Book \*statement/);
    }
}
console.log("citation clipboard feedback tests passed");
