// The smallest DOM search.js will run against: enough to build the result
// list, drive the input, and observe navigation.
export function makeDom() {
  const listeners = new Map();
  const mk = (tag) => {
    const el = {
      tagName: tag, children: [], attrs: {}, dataset: {}, classList: new Set(),
      _text: "", _html: "", handlers: {},
      set textContent(v) { this._text = v; }, get textContent() { return this._text; },
      set innerHTML(v) { this._html = v; if (v === "") el.children.length = 0; },
      get innerHTML() { return this._html; },
      appendChild(c) { el.children.push(c); return c; },
      setAttribute(k, v) { el.attrs[k] = v; },
      getAttribute(k) { return el.attrs[k]; },
      addEventListener(k, fn) { (el.handlers[k] ||= []).push(fn); },
      fire(k, ev = {}) { (el.handlers[k] || []).forEach((fn) => fn(ev)); },
      querySelector(sel) { return el._q(sel); },
      querySelectorAll() { return []; },
      _q(sel) {
        if (sel === "input") return el._input;
        if (sel === ".search-results") return el._list;
        if (sel === ".search-status") return el._status;
        if (sel === "[data-search-close]") return mk("button");
        return null;
      },
      scrollIntoView() {},
    };
    return el;
  };
  const modal = mk("dialog");
  modal.showModal = () => { modal.open = true; };
  modal.close = () => { modal.open = false; };
  modal._input = mk("input");
  modal._input.value = "";
  modal._input.select = () => {};
  modal._list = mk("ul");
  modal._status = mk("p");
  const html = mk("html");
  const document = {
    documentElement: html,
    getElementById: (id) => (id === "search-modal" ? modal : null),
    querySelectorAll: () => [],
    querySelector: () => null,
    createElement: mk,
  };
  const navigated = [];
  const location = {
    pathname: "/", search: "", hash: "",
    set href(v) { navigated.push(v); }, get href() { return navigated.at(-1); },
  };
  return { modal, document, location, navigated, html, listeners };
}
