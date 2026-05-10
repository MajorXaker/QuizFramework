(function () {
  const DEFAULT_LANG = "en";
  const STORAGE_KEY = "quizframework.lang";

  function getCurrentLang() {
    const stored = window.localStorage.getItem(STORAGE_KEY);
    if (stored) return stored;

    const navLang = (navigator.language || navigator.userLanguage || "").toLowerCase();
    if (navLang.startsWith("ru")) return "ru";
    return DEFAULT_LANG;
  }

  function setCurrentLang(lang) {
    window.localStorage.setItem(STORAGE_KEY, lang);
    loadTranslations(lang);
  }

  async function loadTranslations(lang) {
    try {
      const res = await fetch(`/static/i18n/${lang}.json`);
      if (!res.ok) throw new Error("Failed to load translations");
      const data = await res.json();
      window.__translations = data;
      applyTranslations();
    } catch (e) {
      console.error("i18n load error", e);
    }
  }

  function t(key) {
    if (window.__translations && Object.prototype.hasOwnProperty.call(window.__translations, key)) {
      return window.__translations[key];
    }
    return key;
  }

  function applyTranslations() {
    const elements = document.querySelectorAll("[data-i18n]");
    elements.forEach((el) => {
      const key = el.getAttribute("data-i18n");
      const attr = el.getAttribute("data-i18n-attr");
      const value = t(key);
      if (!attr) {
        el.textContent = value;
      } else {
        el.setAttribute(attr, value);
      }
    });
  }

  document.addEventListener("DOMContentLoaded", () => {
    const lang = getCurrentLang();
    loadTranslations(lang);

    document.querySelectorAll("[data-lang-switch]").forEach((btn) => {
      btn.addEventListener("click", () => {
        const lang = btn.getAttribute("data-lang-switch");
        setCurrentLang(lang);
      });
    });
  });

  window.getCurrentLang = getCurrentLang;
  window.setCurrentLang = setCurrentLang;
  window.t = t;
})();
