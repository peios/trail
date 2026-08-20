// Search: a ⌘K modal over the Pagefind bundle that `trail build` writes
// to /pagefind/. The engine and its index chunks load lazily on the first
// open, so pages carry no search cost until someone actually searches.
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

  let pagefind = null;
  const engine = async () => {
    if (!pagefind) {
      pagefind = await import("/pagefind/pagefind.js");
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

  let entries = [];
  let selected = 0;

  const render = () => {
    list.innerHTML = "";
    entries.forEach((entry, index) => {
      const item = document.createElement("li");
      item.setAttribute("role", "option");
      if (index === selected) item.setAttribute("aria-selected", "true");
      const link = document.createElement("a");
      // Carry the query to the page so it can mark the matches.
      link.href = entry.url + "?q=" + encodeURIComponent(input.value.trim());
      const title = document.createElement("p");
      title.className = "result-title";
      title.textContent = entry.meta.title || entry.url;
      link.appendChild(title);
      if (entry.meta.crumbs) {
        const crumbs = document.createElement("p");
        crumbs.className = "result-crumbs";
        crumbs.textContent = entry.meta.crumbs;
        link.appendChild(crumbs);
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

  input.addEventListener("input", async () => {
    const query = input.value.trim();
    if (!query) {
      entries = [];
      render();
      status.textContent = "";
      return;
    }
    const search = await (await engine()).debouncedSearch(query, {}, 120);
    if (search === null) return; // superseded by a newer keystroke
    entries = await Promise.all(search.results.slice(0, 8).map((result) => result.data()));
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
      location.href =
        entries[selected].url + "?q=" + encodeURIComponent(input.value.trim());
    }
  });
})();
