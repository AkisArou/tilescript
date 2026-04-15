const noop = () => {};
const noopSubscription = () => noop;
const emptyState = () => ({
  outputs: [],
  workspaces: [],
  windows: [],
});

export const events = {
  on: noopSubscription,
  once: noopSubscription,
  off: noop,
};

export const wm = {
  spawn: noop,
  reloadConfig: noop,
  setLayout: noop,
  cycleLayout: noop,
  viewWorkspace: noop,
  toggleViewWorkspace: noop,
  toggleFloating: noop,
  toggleFullscreen: noop,
  focusDirection: noop,
  closeWindow: noop,
};

export const query = {
  getState: emptyState,
  getFocusedWindow: () => null,
  getCurrentMonitor: () => null,
  getCurrentWorkspace: () => null,
};
