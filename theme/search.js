// Search: a ⌘K modal over the Pagefind bundle that `trail build` writes
// to <base>/pagefind/. The engine and its index chunks load lazily on the
// first open, so pages carry no search cost until someone actually
// searches. A query starting with "/" is a command rather than a search;
// "/" on its own lists them.
(() => {
  const modal = document.getElementById("search-modal");
  if (!modal || typeof modal.showModal !== "function") return;
  const input = modal.querySelector("input");
  const list = modal.querySelector(".search-results");
  const status = modal.querySelector(".search-status");

  // The pills advertise the real shortcut for the platform.
  if (!/Mac|iPhone|iPad/.test(navigator.platform)) {
    document.querySelectorAll("[data-search-kbd]").forEach((kbd) => {
      kbd.textContent = "Ctrl K";
    });
  }

  // What the build told the page about itself; see base.html. Defaulted
  // so the script still works on a page built before the config existed.
  const site = () => window.trail || { base: "", cachebust: null };

  // ---- Commands ------------------------------------------------------
  // Each command matches its own name and any alias. `run` returns a
  // message to show instead of closing, or nothing to close the modal.

  const themeNames = () => window.trail?.themes || ["system", "light", "dark"];

  const cachebustTarget = () => {
    const { base, cachebust } = site();
    const path = location.pathname;
    const inCopy = base && (path === base || path.startsWith(base + "/"));
    const to = inCopy ? path.slice(base.length) || "/" : "/" + cachebust + path;
    return to + location.search + location.hash;
  };

  const COMMANDS = [
    {
      names: ["/theme"],
      argument: "[system|light|dark]",
      hint: () => `Switch the colour theme — now ${document.documentElement.dataset.theme || "system"}`,
      run: (argument) => {
        if (!argument) {
          window.trail?.cycleTheme?.();
          return;
        }
        if (!window.trail?.setTheme?.(argument)) {
          return `Unknown theme “${argument}” — try ${themeNames().join(", ")}`;
        }
      },
    },
    {
      names: ["/cachebust", "/cb"],
      // Only offered when the build published a copy to jump to.
      available: () => Boolean(site().cachebust),
      hint: () => {
        const { base } = site();
        return base
          ? "Leave the cache-busted copy and return to this page"
          : "Open this page in the cache-busted copy — a URL nothing has cached";
      },
      run: () => {
        location.href = cachebustTarget();
      },
    },
  ];

  /// Split "/theme dark" into its command and argument. Returns null when
  /// the text isn't a command at all, so searching stays the default.
  const parseCommand = (text) => {
    if (!text.startsWith("/")) return null;
    const space = text.indexOf(" ");
    const name = space === -1 ? text : text.slice(0, space);
    const argument = space === -1 ? "" : text.slice(space + 1).trim();
    const available = COMMANDS.filter((command) => command.available?.() !== false);
    // A complete name (or alias) wins outright; otherwise every command
    // the text is a prefix of is offered.
    const exact = available.find((command) => command.names.includes(name));
    const matches = exact
      ? [exact]
      : available.filter((command) => command.names.some((n) => n.startsWith(name)));
    return { name, argument, matches, exact: Boolean(exact) };
  };

  let pagefind = null;
  const engine = async () => {
    if (!pagefind) {
      pagefind = await import(site().base + "/pagefind/pagefind.js");
      await pagefind.init();
    }
    return pagefind;
  };

  const open = () => {
    modal.showModal();
    input.select();
    engine().catch(() => {
      status.textContent = "Search is unavailable (the index failed to load).";
    });
  };

  document.querySelectorAll("[data-search-open]").forEach((button) => {
    button.addEventListener("click", open);
  });
  modal.querySelector("[data-search-close]").addEventListener("click", () => modal.close());
  addEventListener("keydown", (event) => {
    if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
      event.preventDefault();
      modal.open ? modal.close() : open();
    }
  });
  // The panel fills the dialog, so a click landing on the dialog itself
  // can only be on the backdrop.
  modal.addEventListener("click", (event) => {
    if (event.target === modal) modal.close();
  });

  // Both searches and commands land in `entries`, normalised: a title, a
  // second line, an optional excerpt, and either an href to follow or a
  // command to run.
  let entries = [];
  let selected = 0;

  const activate = (entry) => {
    if (entry.href) {
      location.href = entry.href;
      return;
    }
    const message = entry.command.run(entry.argument);
    if (message) {
      status.textContent = message;
      return;
    }
    modal.close();
  };

  const render = () => {
    list.innerHTML = "";
    entries.forEach((entry, index) => {
      const item = document.createElement("li");
      item.setAttribute("role", "option");
      if (index === selected) item.setAttribute("aria-selected", "true");
      const link = document.createElement("a");
      if (entry.href) {
        link.href = entry.href;
      } else {
        link.href = "#";
        link.addEventListener("click", (event) => {
          event.preventDefault();
          activate(entry);
        });
      }
      const title = document.createElement("p");
      title.className = "result-title";
      title.textContent = entry.title;
      link.appendChild(title);
      if (entry.subtitle) {
        const subtitle = document.createElement("p");
        subtitle.className = "result-crumbs";
        subtitle.textContent = entry.subtitle;
        link.appendChild(subtitle);
      }
      if (entry.excerpt) {
        const excerpt = document.createElement("p");
        excerpt.className = "result-excerpt";
        // Pagefind excerpts are escaped text plus <mark> highlights.
        excerpt.innerHTML = entry.excerpt;
        link.appendChild(excerpt);
      }
      item.appendChild(link);
      item.addEventListener("mousemove", () => {
        if (selected !== index) {
          selected = index;
          render();
        }
      });
      list.appendChild(item);
    });
    const current = list.querySelector('[aria-selected="true"]');
    if (current) current.scrollIntoView({ block: "nearest" });
  };

  const showCommands = (parsed) => {
    entries = parsed.matches.map((command) => ({
      title: command.names.join(", ") + (command.argument ? " " + command.argument : ""),
      subtitle: command.hint(),
      command,
      argument: parsed.exact ? parsed.argument : "",
    }));
    selected = 0;
    render();
    status.textContent = entries.length
      ? ""
      : `No command matches “${parsed.name}” — type / to see them all`;
  };

  input.addEventListener("input", async () => {
    const query = input.value.trim();
    if (!query) {
      entries = [];
      render();
      status.textContent = "";
      return;
    }
    const parsed = parseCommand(query);
    if (parsed) {
      showCommands(parsed);
      return;
    }
    const search = await (await engine()).debouncedSearch(query, {}, 120);
    if (search === null) return; // superseded by a newer keystroke
    const results = await Promise.all(
      search.results.slice(0, 8).map((result) => result.data()),
    );
    entries = results.map((result) => ({
      title: result.meta.title || result.url,
      subtitle: result.meta.crumbs,
      excerpt: result.excerpt,
      // Carry the query to the page so it can mark the matches.
      href: result.url + "?q=" + encodeURIComponent(query),
    }));
    selected = 0;
    render();
    if (search.results.length === 0) {
      status.textContent = `No results for “${query}”`;
    } else if (search.results.length > entries.length) {
      status.textContent = `Showing ${entries.length} of ${search.results.length} results`;
    } else {
      status.textContent = "";
    }
  });

  input.addEventListener("keydown", (event) => {
    if (event.key === "ArrowDown") {
      event.preventDefault();
      selected = Math.min(selected + 1, entries.length - 1);
      render();
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      selected = Math.max(selected - 1, 0);
      render();
    } else if (event.key === "Enter" && entries[selected]) {
      event.preventDefault();
      activate(entries[selected]);
    }
  });
})();
