// Appended only by the loopback update fixture, after the production worker body.
(() => {
  const revision = "fixture-revision";
  const started = performance.now();
  let pending = 0;
  let pendingResponses = 0;
  let pendingHandled = 0;
  const record = (operation, data = {}) =>
    console.debug(
      "offline-update-probe:" +
        JSON.stringify({
          type: "producer",
          operation,
          revision,
          at: Date.now(),
          bootAt: performance.timeOrigin,
          elapsed_ms: Math.round(performance.now() - started),
          pending,
          pendingResponses,
          pendingHandled,
          ...data,
        }),
    );
  record("boot");
  const observePromise = (promise, fulfilled, rejected) => {
    try {
      // Never assimilate a thenable a second time after the native Web IDL call.
      void Promise.prototype.then.call(promise, fulfilled, rejected);
      return true;
    } catch {
      return false;
    }
  };
  const skipWaiting = self.skipWaiting.bind(self);
  self.skipWaiting = () => {
    record("skipWaiting-request");
    const result = skipWaiting();
    void result.then(
      () => record("skipWaiting-resolved"),
      () => record("skipWaiting-rejected"),
    );
    return result;
  };
  for (const method of ["get", "matchAll"]) {
    const original = self.clients[method].bind(self.clients);
    self.clients[method] = (...args) => {
      record(`${method}-request`);
      const result = original(...args);
      void result.then(
        (clients) =>
          record(`${method}-resolved`, {
            ids: (Array.isArray(clients) ? clients : [clients])
              .filter(Boolean)
              .map((client) => client.id),
          }),
        () => record(`${method}-rejected`),
      );
      return result;
    };
  }
  const waitUntil = ExtendableEvent.prototype.waitUntil;
  ExtendableEvent.prototype.waitUntil = function (promise) {
    let result;
    const event = closed(this.type, [
      "fetch",
      "message",
      "install",
      "activate",
    ]);
    try {
      result = Reflect.apply(waitUntil, this, [promise]);
    } catch (error) {
      record("waitUntil-threw", { event });
      throw error;
    }
    pending++;
    record("waitUntil-request", { event, pending });
    const settled = () => {
      pending--;
      record("waitUntil-settled", { event, pending });
    };
    if (!observePromise(promise, settled, settled)) {
      pending--;
      record("waitUntil-unavailable", { event });
    }
    return result;
  };
  const events = new WeakMap();
  let sequence = 0;
  const closed = (value, allowed) =>
    allowed.includes(value) ? value : "other";
  const observeFetch = (event) => {
    const previous = events.get(event);
    if (previous) return previous;
    const info = {
      eventId: ++sequence,
      method: closed(event.request.method, ["GET", "HEAD", "POST"]),
      mode: closed(event.request.mode, [
        "navigate",
        "same-origin",
        "cors",
        "no-cors",
      ]),
      destination: closed(event.request.destination, [
        "",
        "document",
        "script",
        "style",
        "image",
        "font",
        "worker",
      ]),
    };
    events.set(event, info);
    const handled = event.handled;
    pendingHandled++;
    if (
      !observePromise(
        handled,
        () => {
          pendingHandled--;
          record("handled-fulfilled", info);
        },
        () => {
          pendingHandled--;
          record("handled-rejected", info);
        },
      )
    ) {
      pendingHandled--;
      record("handled-unavailable", info);
    }
    record("fetch-event", info);
    return info;
  };
  const respondWith = FetchEvent.prototype.respondWith;
  FetchEvent.prototype.respondWith = function (promise) {
    let result;
    try {
      result = Reflect.apply(respondWith, this, [promise]);
    } catch (error) {
      record("respondWith-threw");
      throw error;
    }
    const info = observeFetch(this);
    pendingResponses++;
    record("respondWith-request", info);
    if (
      !observePromise(
        promise,
        () => {
          pendingResponses--;
          record("respondWith-fulfilled", info);
        },
        () => {
          pendingResponses--;
          record("respondWith-rejected", info);
        },
      )
    ) {
      pendingResponses--;
      record("respondWith-unavailable", info);
    }
    return result;
  };
  self.addEventListener("fetch", observeFetch);
  self.addEventListener("message", (event) => {
    if (!["update", "clear-cache"].includes(event.data?.type)) return;
    record("command", { command: event.data.type });
    const port = event.ports[0];
    if (!port) return;
    const send = port.postMessage.bind(port);
    port.postMessage = (value) => {
      record("command-reply", { command: event.data.type, value });
      send(value);
    };
  });
})();
