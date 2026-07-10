/**
 * weavr interactivity — self-contained, no external dependencies.
 *
 * Works on both the index page (project list) and per-project page
 * (session list).  Detects page context from the DOM.
 *
 * Features:
 *   1. View-mode toggle (cards / list) — index page only
 *   2. Search bar (case-insensitive substring, 150ms debounce)
 *   3. Date filter (preset chips + custom range picker)
 *
 * All filters combine via AND.  Inline theme + view-mode scripts in
 * <head> prevent layout flash before the full script boots.
 */
(function () {
  'use strict';

  // ---- Page context detection ----

  var isIndexPage = !!document.querySelector('.project-grid');
  var isProjectPage = !!document.querySelector('.session-list');

  var CARD_SELECTOR = isProjectPage ? '.session-card' : '.project-card';
  var DATE_ATTR = isProjectPage ? 'data-started-at' : 'data-last-activity';

  // ---- Search attributes (checked in order) ----
  var SEARCH_ATTRS = isProjectPage
    ? ['data-prompt', 'data-title', 'data-id']
    : ['data-display-name', 'data-short-name', 'data-name'];

  var VIEW_KEY = 'weavr:index:viewMode';

  // -----------------------------------------------------------------------
  // 1. View-mode toggle (index page only)
  // -----------------------------------------------------------------------

  function getViewMode() {
    try {
      return localStorage.getItem(VIEW_KEY) || 'cards';
    } catch (_) {
      return 'cards';
    }
  }

  function setViewMode(mode) {
    try {
      localStorage.setItem(VIEW_KEY, mode);
    } catch (_) { /* quota exceeded — degrade gracefully */ }
  }

  function applyViewMode(mode) {
    var grid = document.querySelector('.project-grid');
    if (!grid) return;
    grid.setAttribute('data-view', mode);
    // Update both view-switcher buttons.
    var buttons = document.querySelectorAll('[data-view-mode]');
    buttons.forEach(function (btn) {
      var btnMode = btn.getAttribute('data-view-mode');
      btn.setAttribute('aria-pressed', btnMode === mode ? 'true' : 'false');
    });
  }

  // ---- List-view column sorting ----

  var currentSort = { col: null, asc: true };

  function getSortValue(card, col) {
    switch (col) {
      case 'name':
        return (card.getAttribute('data-short-name') || '').toLowerCase();
      case 'activity':
        return card.getAttribute('data-last-activity') || '';
      case 'sessions':
        return parseInt(card.getAttribute('data-sessions') || '0', 10);
      case 'messages':
        return parseInt(card.getAttribute('data-messages') || '0', 10);
      case 'tokens':
        return parseInt(card.getAttribute('data-tokens-raw') || '0', 10);
      default:
        return '';
    }
  }

  function sortCards(col, asc) {
    var grid = document.querySelector('.project-grid');
    if (!grid) return;
    var cards = Array.from(grid.querySelectorAll('.project-card'));
    cards.sort(function (a, b) {
      var va = getSortValue(a, col);
      var vb = getSortValue(b, col);
      if (typeof va === 'string') {
        return asc ? va.localeCompare(vb) : vb.localeCompare(va);
      }
      return asc ? va - vb : vb - va;
    });
    cards.forEach(function (c) { grid.appendChild(c); });
  }

  function updateSortHeaders() {
    var headers = document.querySelectorAll('[data-sort-col]');
    headers.forEach(function (h) {
      var col = h.getAttribute('data-sort-col');
      h.classList.remove('sort-asc', 'sort-desc');
      if (col === currentSort.col) {
        h.classList.add(currentSort.asc ? 'sort-asc' : 'sort-desc');
      }
    });
  }

  function initViewToggle() {
    var mode = getViewMode();
    applyViewMode(mode);

    var buttons = document.querySelectorAll('[data-view-mode]');
    if (!buttons.length) return;

    buttons.forEach(function (btn) {
      btn.addEventListener('click', function () {
        var next = this.getAttribute('data-view-mode');
        if (!next) return;
        setViewMode(next);
        applyViewMode(next);
        // Re-apply sort if switching to list view.
        if (next === 'list' && currentSort.col) {
          sortCards(currentSort.col, currentSort.asc);
          updateSortHeaders();
        }
      });
    });
  }

  function initSortableHeaders() {
    var headers = document.querySelectorAll('[data-sort-col]');
    headers.forEach(function (h) {
      h.addEventListener('click', function () {
        var col = h.getAttribute('data-sort-col');
        var asc = col === currentSort.col ? !currentSort.asc : true;
        currentSort = { col: col, asc: asc };
        sortCards(col, asc);
        updateSortHeaders();
      });
    });
  }

  // -----------------------------------------------------------------------
  // 2. Search bar
  // -----------------------------------------------------------------------

  var searchDebounce = null;
  var currentSearch = '';

  function applySearch(term) {
    currentSearch = term.toLowerCase();
    var cards = document.querySelectorAll(CARD_SELECTOR);
    cards.forEach(function (c) {
      if (!currentSearch) {
        c.removeAttribute('data-search-hidden');
        return;
      }
      var match = false;
      for (var i = 0; i < SEARCH_ATTRS.length; i++) {
        var val = (c.getAttribute(SEARCH_ATTRS[i]) || '').toLowerCase();
        if (val.indexOf(currentSearch) !== -1) {
          match = true;
          break;
        }
      }
      if (match) {
        c.removeAttribute('data-search-hidden');
      } else {
        c.setAttribute('data-search-hidden', '');
      }
    });
    syncVisibility();
  }

  function initSearch() {
    var input = document.querySelector('.index-search-input');
    if (!input) return;

    input.addEventListener('input', function () {
      var term = this.value;
      clearTimeout(searchDebounce);
      searchDebounce = setTimeout(function () {
        applySearch(term);
      }, 150);
    });
  }

  // -----------------------------------------------------------------------
  // 3. Date filter
  // -----------------------------------------------------------------------

  var currentDatePreset = 'all';
  var currentDateFrom = null;
  var currentDateTo = null;

  function toLocalDate(iso) {
    if (!iso) return null;
    // Parse ISO-8601 and convert to local date string YYYY-MM-DD.
    try {
      var d = new Date(iso);
      if (isNaN(d.getTime())) return null;
      return d.getFullYear() + '-' +
        String(d.getMonth() + 1).padStart(2, '0') + '-' +
        String(d.getDate()).padStart(2, '0');
    } catch (_) {
      return null;
    }
  }

  function todayStr() {
    var d = new Date();
    return d.getFullYear() + '-' +
      String(d.getMonth() + 1).padStart(2, '0') + '-' +
      String(d.getDate()).padStart(2, '0');
  }

  function daysAgoStr(n) {
    var d = new Date();
    d.setDate(d.getDate() - n);
    return d.getFullYear() + '-' +
      String(d.getMonth() + 1).padStart(2, '0') + '-' +
      String(d.getDate()).padStart(2, '0');
  }

  function applyDateFilter() {
    var cards = document.querySelectorAll(CARD_SELECTOR);
    cards.forEach(function (c) {
      // No filter → visible.
      if (currentDatePreset === 'all' && !currentDateFrom && !currentDateTo) {
        c.removeAttribute('data-date-hidden');
        return;
      }
      var activityIso = c.getAttribute(DATE_ATTR);
      var activityDate = toLocalDate(activityIso);
      if (!activityDate) {
        // No timestamp → show if "All time", hide otherwise.
        if (currentDatePreset === 'all' && !currentDateFrom && !currentDateTo) {
          c.removeAttribute('data-date-hidden');
        } else {
          c.setAttribute('data-date-hidden', '');
        }
        return;
      }

      var from = currentDateFrom;
      var to = currentDateTo;
      if (currentDatePreset === 'today') {
        from = todayStr();
        to = todayStr();
      } else if (currentDatePreset === '7days') {
        from = daysAgoStr(7);
        to = todayStr();
      } else if (currentDatePreset === '30days') {
        from = daysAgoStr(30);
        to = todayStr();
      }

      var visible = true;
      if (from && activityDate < from) visible = false;
      if (to && activityDate > to) visible = false;
      if (visible) {
        c.removeAttribute('data-date-hidden');
      } else {
        c.setAttribute('data-date-hidden', '');
      }
    });
    syncVisibility();
  }

  function syncDateInputs() {
    var fromInput = document.querySelector('.date-picker-from');
    var toInput = document.querySelector('.date-picker-to');
    var now = new Date();
    if (currentDatePreset === 'today') {
      if (fromInput) fromInput.value = todayStr();
      if (toInput) toInput.value = todayStr();
    } else if (currentDatePreset === '7days') {
      if (fromInput) fromInput.value = daysAgoStr(7);
      if (toInput) toInput.value = todayStr();
    } else if (currentDatePreset === '30days') {
      if (fromInput) fromInput.value = daysAgoStr(30);
      if (toInput) toInput.value = todayStr();
    } else if (currentDatePreset === 'all') {
      if (fromInput) fromInput.value = '';
      if (toInput) toInput.value = '';
    }
  }

  function updateDateChips(active) {
    var chips = document.querySelectorAll('[data-date-preset]');
    chips.forEach(function (chip) {
      var preset = chip.getAttribute('data-date-preset');
      if (preset === active) {
        chip.classList.add('date-chip--active');
        chip.setAttribute('aria-pressed', 'true');
      } else {
        chip.classList.remove('date-chip--active');
        chip.setAttribute('aria-pressed', 'false');
      }
    });
  }

  function initDateFilter() {
    // Preset chips.
    var chips = document.querySelectorAll('[data-date-preset]');
    chips.forEach(function (chip) {
      chip.addEventListener('click', function () {
        currentDatePreset = this.getAttribute('data-date-preset');
        currentDateFrom = null;
        currentDateTo = null;
        syncDateInputs();
        updateDateChips(currentDatePreset);
        applyDateFilter();
      });
    });

    // Custom range inputs.
    var fromInput = document.querySelector('.date-picker-from');
    var toInput = document.querySelector('.date-picker-to');
    var applyBtn = document.querySelector('.date-picker-apply');

    function onCustomRange() {
      currentDatePreset = 'custom';
      currentDateFrom = fromInput ? fromInput.value || null : null;
      currentDateTo = toInput ? toInput.value || null : null;
      updateDateChips('custom');
      applyDateFilter();
    }

    if (fromInput) fromInput.addEventListener('change', onCustomRange);
    if (toInput) toInput.addEventListener('change', onCustomRange);
    if (applyBtn) {
      applyBtn.addEventListener('click', function () {
        currentDatePreset = 'custom';
        currentDateFrom = fromInput ? fromInput.value || null : null;
        currentDateTo = toInput ? toInput.value || null : null;
        updateDateChips('custom');
        applyDateFilter();
      });
    }

    // Initial state: "All time" active.
    updateDateChips('all');
  }

  // -----------------------------------------------------------------------
  // Combined visibility
  // -----------------------------------------------------------------------

  function syncVisibility() {
    var cards = document.querySelectorAll(CARD_SELECTOR);
    var visibleCount = 0;
    cards.forEach(function (c) {
      var searchHidden = c.hasAttribute('data-search-hidden');
      var dateHidden = c.hasAttribute('data-date-hidden');
      var hidden = searchHidden || dateHidden;
      c.hidden = hidden;
      if (!hidden) visibleCount++;
    });
    // Show/hide empty state.
    var empty = document.querySelector('.empty-state');
    if (empty) {
      empty.hidden = visibleCount > 0 || cards.length === 0;
    }
  }

  // -----------------------------------------------------------------------
  // 4. Theme toggle
  // -----------------------------------------------------------------------

  function initThemeToggle() {
    var btn = document.getElementById('theme-toggle');
    if (!btn) return;

    function updateIcon() {
      var theme = document.documentElement.getAttribute('data-theme');
      btn.innerHTML = theme === 'light' ? '&#x2600;' : '&#x25D0;';
    }

    btn.addEventListener('click', function () {
      var current = document.documentElement.getAttribute('data-theme');
      var next = current === 'light' ? 'dark' : 'light';
      document.documentElement.setAttribute('data-theme', next);
      try { localStorage.setItem('weavr-theme', next); } catch (_) {}
      updateIcon();
    });

    updateIcon();
  }

  // -----------------------------------------------------------------------
  // Tooltip (data-tooltip attribute)
  // -----------------------------------------------------------------------

  function initTooltip() {
    var tip = document.createElement('div');
    tip.className = 'weavr-tooltip';
    tip.hidden = true;
    document.body.appendChild(tip);

    document.addEventListener('mouseover', function (e) {
      var target = e.target.closest('[data-tooltip]');
      if (!target) return;
      tip.textContent = target.getAttribute('data-tooltip');
      tip.hidden = false;
    });

    document.addEventListener('mousemove', function (e) {
      if (tip.hidden) return;
      tip.style.left = (e.clientX + 12) + 'px';
      tip.style.top  = (e.clientY + 12) + 'px';
    });

    document.addEventListener('mouseout', function (e) {
      var target = e.target.closest('[data-tooltip]');
      if (!target) return;
      tip.hidden = true;
    });
  }

  // -----------------------------------------------------------------------
  // Boot
  // -----------------------------------------------------------------------

  function init() {
    // Index-only features.
    if (isIndexPage) {
      initViewToggle();
      initSortableHeaders();
    }
    // Common features (both pages).
    initSearch();
    initDateFilter();
    initThemeToggle();
    initTooltip();
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', init);
  } else {
    init();
  }
})();
