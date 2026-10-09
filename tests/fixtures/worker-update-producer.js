// Appended only by the loopback update fixture, after the production worker body.
(() => {
  const revision = "fixture-revision";
  const record = (operation, data = {}) =>
    console.debug(
      "offline-update-probe:" +
        JSON.stringify({
          type: "producer",
          operation,
          revision,
          at: Date.now(),
          ...data,
        }),
    );
  record("boot");
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
  let pending = 0;
  const waitUntil = ExtendableEvent.prototype.waitUntil;
  ExtendableEvent.prototype.waitUntil = function (promise) {
    pending++;
    record("waitUntil-request", { event: this.type, pending });
    const settled = () => {
      pending--;
      record("waitUntil-settled", { event: this.type, pending });
    };
    void Promise.resolve(promise).then(settled, settled);
    return Reflect.apply(waitUntil, this, [promise]);
  };
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
