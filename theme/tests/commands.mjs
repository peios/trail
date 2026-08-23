// The slash commands in theme/search.js, driven against a stub DOM small
// enough to live here (see dom.mjs). `cargo test` runs this through
// node when node is installed.
import fs from "node:fs";
import vm from "node:vm";
import { makeDom } from "./dom.mjs";

// Run from the repository root: `node theme/tests/commands.mjs`.
const source = fs.readFileSync(
  process.argv[2] || new URL("../search.js", import.meta.url),
  "utf8",
);

function run({ base, cachebust, pathname }) {
  const dom = makeDom();
  dom.location.pathname = pathname;
  const themes = ["system", "light", "dark"];
  let themeState = "system";
  const win = {
    trail: {
      base, cachebust, themes,
      setTheme: (s) => (themes.includes(s) ? ((themeState = s), s) : null),
      cycleTheme: () => (themeState = themes[(themes.indexOf(themeState) + 1) % 3]),
    },
    navigator: { platform: "Linux x86_64" },
    document: dom.document,
    location: dom.location,
    addEventListener: () => {},
  };
  win.window = win;
  vm.createContext(win);
  vm.runInContext(source, win);
  return {
    dom,
    theme: () => themeState,
    type: (text) => { dom.modal._input.value = text; return dom.modal._input.fire("input"); },
    enter: () => dom.modal._input.fire("keydown", { key: "Enter", preventDefault() {} }),
    titles: () => dom.modal._list.children.map((li) => li.children[0].children[0].textContent),
    subs: () => dom.modal._list.children.map((li) => li.children[0].children[1]?.textContent),
    status: () => dom.modal._status.textContent,
  };
}

let failures = 0;
const check = (name, actual, expected) => {
  const ok = JSON.stringify(actual) === JSON.stringify(expected);
  if (!ok) { failures++; console.log(`FAIL ${name}\n  got      ${JSON.stringify(actual)}\n  expected ${JSON.stringify(expected)}`); }
  else console.log(`ok   ${name}`);
};

// --- On the site itself, with a copy published ---------------------------
let t = run({ base: "", cachebust: "0a1b2c3d", pathname: "/peios/using-peios/install/" });
await t.type("/");
check("/ lists every command", t.titles(), ["/theme [system|light|dark]", "/cachebust, /cb"]);
await t.type("/th");
check("/th narrows to the theme command", t.titles(), ["/theme [system|light|dark]"]);
await t.type("/theme dark");
t.enter();
check("/theme dark sets the theme", t.theme(), "dark");
check("running a command closes the modal", t.dom.modal.open, false);
await t.type("/theme");
t.enter();
check("bare /theme cycles from dark", t.theme(), "system");
await t.type("/theme mauve");
t.enter();
check("an unknown theme explains itself", t.status(), "Unknown theme “mauve” — try system, light, dark");
check("and leaves the theme alone", t.theme(), "system");
await t.type("/cb");
t.enter();
check("/cb enters the copy at this page", t.dom.navigated, ["/0a1b2c3d/peios/using-peios/install/"]);
await t.type("/nope");
check("an unknown command says so", t.status(), "No command matches “/nope” — type / to see them all");

// --- Inside the copy ------------------------------------------------------
t = run({ base: "/0a1b2c3d", cachebust: "0a1b2c3d", pathname: "/0a1b2c3d/peios/using-peios/install/" });
await t.type("/cb");
t.enter();
check("/cb leaves the copy again", t.dom.navigated, ["/peios/using-peios/install/"]);
await t.type("/");
check("the hint says which way it goes", t.subs()[1], "Leave the cache-busted copy and return to this page");
t = run({ base: "/0a1b2c3d", cachebust: "0a1b2c3d", pathname: "/0a1b2c3d/" });
await t.type("/cb");
t.enter();
check("/cb from the copy's front page", t.dom.navigated, ["/"]);

// --- With no copy published ----------------------------------------------
t = run({ base: "", cachebust: null, pathname: "/" });
await t.type("/");
check("/cb is not offered when there is no copy", t.titles(), ["/theme [system|light|dark]"]);

// --- Query and fragment survive the jump ---------------------------------
t = run({ base: "", cachebust: "abc", pathname: "/peios/x/" });
t.dom.location.search = "?q=boot";
t.dom.location.hash = "#step-2";
await t.type("/cb");
t.enter();
check("the query and fragment come along", t.dom.navigated, ["/abc/peios/x/?q=boot#step-2"]);

console.log(failures ? `\n${failures} failing` : "\nall passing");
process.exit(failures ? 1 : 0);
