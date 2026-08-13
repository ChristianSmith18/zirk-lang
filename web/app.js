(function () {
  'use strict';
  const root = document.documentElement;
  const themeKey = 'zirk-docs-theme';
  const themeSelect = document.querySelector('[data-theme-select]');
  const saved = localStorage.getItem(themeKey) || 'system';

  function applyTheme(value) {
    const resolved = value === 'system'
      ? (matchMedia('(prefers-color-scheme: light)').matches ? 'light' : 'dark')
      : value;
    root.dataset.theme = resolved;
    if (themeSelect) themeSelect.value = value;
  }
  applyTheme(saved);
  themeSelect?.addEventListener('change', (event) => {
    localStorage.setItem(themeKey, event.target.value);
    applyTheme(event.target.value);
  });
  matchMedia('(prefers-color-scheme: light)').addEventListener?.('change', () => {
    if ((localStorage.getItem(themeKey) || 'system') === 'system') applyTheme('system');
  });

  const menuButton = document.querySelector('[data-menu-trigger]');
  const mobileNav = document.querySelector('[data-mobile-nav]');
  function closeMenu() {
    if (!mobileNav) return;
    mobileNav.hidden = true;
    menuButton?.setAttribute('aria-expanded', 'false');
  }
  menuButton?.addEventListener('click', () => {
    const opening = mobileNav.hidden;
    mobileNav.hidden = !opening;
    menuButton.setAttribute('aria-expanded', String(opening));
    if (opening) mobileNav.querySelector('a')?.focus();
  });
  mobileNav?.addEventListener('click', (event) => { if (event.target.closest('a')) closeMenu(); });

  document.querySelectorAll('pre').forEach((pre) => {
    const wrapper = pre.parentElement;
    if (!wrapper?.classList.contains('workbench') && !wrapper?.classList.contains('code-block')) return;
    const head = wrapper.querySelector('.code-head');
    if (!head || head.querySelector('.copy-button')) return;
    const button = document.createElement('button');
    button.type = 'button'; button.className = 'copy-button'; button.textContent = 'Copy';
    button.setAttribute('aria-label', 'Copy code example');
    button.addEventListener('click', async () => {
      await navigator.clipboard.writeText(pre.innerText);
      button.textContent = 'Copied'; button.setAttribute('aria-label', 'Code copied');
      setTimeout(() => { button.textContent = 'Copy'; button.setAttribute('aria-label', 'Copy code example'); }, 1600);
    });
    head.append(button);
  });

  const tocLinks = [...document.querySelectorAll('.toc a[href^="#"]')];
  const sections = tocLinks.map((link) => document.querySelector(link.getAttribute('href'))).filter(Boolean);
  if ('IntersectionObserver' in window && sections.length) {
    const observer = new IntersectionObserver((entries) => {
      const visible = entries.filter((entry) => entry.isIntersecting).sort((a,b) => b.intersectionRatio-a.intersectionRatio)[0];
      if (!visible) return;
      tocLinks.forEach((link) => link.classList.toggle('active', link.hash === `#${visible.target.id}`));
    }, { rootMargin: '-20% 0px -65%', threshold: [0,.25,.75] });
    sections.forEach((section) => observer.observe(section));
  }

  const dialog = document.querySelector('[data-search-dialog]');
  const searchInput = dialog?.querySelector('input');
  const results = dialog?.querySelector('[data-search-results]');
  let index = [];
  async function openSearch() {
    dialog.showModal(); searchInput.focus();
    if (!index.length) index = await fetch('search-index.json').then((response) => response.json());
    renderResults('');
  }
  function renderResults(query) {
    const terms = query.toLowerCase().trim().split(/\s+/).filter(Boolean);
    const matches = index.filter((item) => terms.every((term) => `${item.title} ${item.summary} ${item.keywords}`.toLowerCase().includes(term))).slice(0,12);
    results.textContent = '';
    if (!matches.length) { results.innerHTML = '<li><span>No results. Try “task”, “types”, or “permissions”.</span></li>'; return; }
    matches.forEach((item) => {
      const li = document.createElement('li'); const link = document.createElement('a');
      link.href = item.url; link.innerHTML = `<strong>${item.title}</strong><span>${item.section} · ${item.summary}</span>`;
      li.append(link); results.append(li);
    });
  }
  document.querySelectorAll('[data-search-trigger]').forEach((button) => button.addEventListener('click', openSearch));
  searchInput?.addEventListener('input', () => renderResults(searchInput.value));
  dialog?.querySelector('[data-search-close]')?.addEventListener('click', () => dialog.close());
  document.addEventListener('keydown', (event) => {
    if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'k') { event.preventDefault(); openSearch(); }
    if (event.key === 'Escape') closeMenu();
  });
})();
