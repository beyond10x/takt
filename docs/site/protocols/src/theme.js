// The reader's colour mode, shared with the documentation site. The site keeps the choice ("light",
// "dark", or none for the system's scheme) in localStorage under its key, and the page's first
// script sets data-theme and data-theme-choice on <html> from it (vite.config.js), as the site's
// own pages do. This module keeps both attributes current while the page is open: a choice made
// on a docs page in another tab, a change of the system's scheme, and the viewer's own toggle.
const KEY = __SITE__.themeKey;
const system = window.matchMedia("(prefers-color-scheme: dark)");

function stored() {
  try {
    const value = localStorage.getItem(KEY);
    return value === "light" || value === "dark" ? value : null;
  } catch {
    return null;
  }
}

function apply(choice) {
  const root = document.documentElement;
  root.setAttribute("data-theme", choice ?? (system.matches ? "dark" : "light"));
  root.setAttribute("data-theme-choice", choice ?? "system");
}

export const choice = () => document.documentElement.getAttribute("data-theme-choice") || "system";
export const dark = () => document.documentElement.getAttribute("data-theme") === "dark";

// The site's toggle order: the system's scheme, then light, then dark, then the system's again.
export function next() {
  const now = stored();
  const then = now === null ? "light" : now === "light" ? "dark" : null;
  try {
    if (then) localStorage.setItem(KEY, then);
    else localStorage.removeItem(KEY);
  } catch {
    // a browser that refuses storage still switches this page
  }
  apply(then);
}

export function follow() {
  window.addEventListener("storage", (e) => {
    if (e.key === KEY || e.key === null) apply(stored());
  });
  system.addEventListener("change", () => {
    if (stored() === null) apply(null);
  });
}
