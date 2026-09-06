const styledRoots = new WeakSet();
const installedViewers = new WeakMap();

// The Prism property panels live across several nested custom-element shadow
// roots. Keep this adapter outside the vendor bundle and install the same
// small-screen treatment at each root as it appears.
const MOBILE_PROPERTIES_CSS = `
@media (max-width: 640px) {
  .bottom-left-icon {
    top: 8px !important;
    bottom: auto !important;
  }

  kc-board-properties-panel,
  kc-schematic-properties-panel {
    position: fixed !important;
    left: 8px !important;
    right: 8px !important;
    top: auto !important;
    bottom: 8px !important;
    width: auto !important;
    height: min(48vh, 360px) !important;
    max-height: calc(100vh - 16px) !important;
    min-height: 0 !important;
    z-index: 40 !important;
    align-items: stretch !important;
    border-radius: 12px !important;
    overflow: hidden !important;
    box-shadow: 0 10px 30px rgb(0 0 0 / 0.35) !important;
  }

  kc-board-properties-panel[hidden],
  kc-schematic-properties-panel[hidden] {
    display: none !important;
  }

  kc-ui-panel {
    width: 100% !important;
    max-width: none !important;
    min-height: 0 !important;
    max-height: 100% !important;
    display: flex !important;
    flex-direction: column !important;
  }

  kc-ui-panel-body {
    min-height: 0 !important;
    overflow: auto !important;
    -webkit-overflow-scrolling: touch !important;
  }

  kc-ui-property-list {
    font-size: 14px !important;
    line-height: 1.3 !important;
    grid-template-columns: minmax(7rem, 42%) minmax(0, 1fr) !important;
  }

  kc-ui-panel-title-with-close {
    min-height: 2.75rem !important;
  }

  kc-ui-button[variant="close"] {
    min-width: 44px !important;
    min-height: 44px !important;
  }

  kc-ui-button[variant="close"]::part(base) {
    min-width: 44px !important;
    min-height: 44px !important;
  }
}
`;

const MOBILE_PROPERTY_ITEM_CSS = `
@media (max-width: 640px) {
  :host {
    font-size: 14px !important;
    line-height: 1.3 !important;
  }

  :host span {
    min-height: 2.25rem !important;
    padding: 0.55rem 0.6rem !important;
    font-size: 14px !important;
    line-height: 1.3 !important;
    white-space: normal !important;
    overflow-wrap: anywhere !important;
    word-break: break-word !important;
    text-overflow: clip !important;
  }

  :host(.label) span:first-of-type {
    grid-column: 1 / -1;
    min-height: 2.5rem !important;
    font-weight: 700 !important;
  }

  :host(.label) span:nth-of-type(2) {
    display: none !important;
  }
}
`;

const panelSheet = new CSSStyleSheet();
panelSheet.replaceSync(MOBILE_PROPERTIES_CSS);
const itemSheet = new CSSStyleSheet();
itemSheet.replaceSync(MOBILE_PROPERTY_ITEM_CSS);

function installStyle(root) {
  if (styledRoots.has(root)) return;
  styledRoots.add(root);
  // Keep Prism's DOM intact, including its first/last-child property-row selectors.
  root.adoptedStyleSheets = [
    ...root.adoptedStyleSheets,
    root.host?.localName === "kc-ui-property-list-item" ? itemSheet : panelSheet,
  ];
}

export function installMobileProperties(viewer) {
  if (!viewer.shadowRoot) return;
  const refresh = installedViewers.get(viewer);
  if (refresh) {
    refresh();
    return;
  }
  const observedRoots = new WeakSet();
  let refreshQueued = false;
  const visit = (root) => {
    installStyle(root);
    if (!observedRoots.has(root)) {
      observedRoots.add(root);
      new MutationObserver(() => {
        if (refreshQueued) return;
        refreshQueued = true;
        requestAnimationFrame(() => {
          refreshQueued = false;
          if (viewer.shadowRoot) visit(viewer.shadowRoot);
        });
      }).observe(root, {
        childList: true,
        subtree: true,
      });
    }
    for (const element of root.querySelectorAll("*")) {
      if (element.shadowRoot) visit(element.shadowRoot);
    }
  };
  const refreshViewer = () => {
    if (viewer.shadowRoot) visit(viewer.shadowRoot);
  };
  installedViewers.set(viewer, refreshViewer);
  refreshViewer();
}
