import { readFileSync } from "node:fs";
import { runInNewContext } from "node:vm";
import assert from "node:assert/strict";
import { test } from "node:test";

const source = readFileSync(
  new URL("./worker-update-producer.js", import.meta.url),
  "utf8",
);
const deferred = () => {
  let resolve, reject;
  const promise = new Promise((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
};
const tick = () => new Promise((resolve) => setImmediate(resolve));
function fixture() {
  const records = [];
  const handlers = new Map();
  const result = {};
  const failure = Error("credential-must-not-be-logged");
  class ExtendableEvent {
    constructor(type) {
      this.type = type;
      this.lifetimes = [];
    }
    waitUntil(promise) {
      if (this.invalid) throw failure;
      this.lifetimes.push(promise);
      return result;
    }
  }
  class FetchEvent extends ExtendableEvent {
    constructor() {
      super("fetch");
      this.completion = deferred();
      this.handled = this.completion.promise;
      this.request = {
        method: "GET",
        mode: "navigate",
        destination: "document",
        url: "https://private.invalid/?token=must-not-be-logged",
        headers: { cookie: "must-not-be-logged" },
      };
    }
    respondWith(promise) {
      if (this.invalid) throw failure;
      this.response = promise;
      this.nativeResponse = Promise.resolve(promise);
      return result;
    }
  }
  let clock = 0;
  const self = {
    skipWaiting: () => Promise.resolve(),
    clients: {
      get: () => Promise.resolve(),
      matchAll: () => Promise.resolve([]),
    },
    addEventListener: (type, handler) => {
      const group = handlers.get(type) ?? [];
      group.push(handler);
      handlers.set(type, group);
    },
  };
  runInNewContext(source, {
    self,
    ExtendableEvent,
    FetchEvent,
    Promise,
    performance: { now: () => ++clock, timeOrigin: 1000 },
    console: {
      debug: (line) =>
        records.push(JSON.parse(line.split("offline-update-probe:")[1])),
    },
  });
  const dispatch = (event) => {
    for (const handler of handlers.get(event.type) ?? []) handler(event);
  };
  return {
    records,
    result,
    failure,
    FetchEvent,
    ExtendableEvent,
    dispatch,
    self,
  };
}
function last(records, operation) {
  return records.findLast((row) => row.operation === operation);
}

test("a thenable is assimilated only by the native call and direct Response stays untouched", async () => {
  const f = fixture();
  let calls = 0;
  const response = new Response("once");
  const input = {
    then(resolve) {
      calls++;
      resolve(response);
    },
  };
  const event = new f.FetchEvent();
  assert.equal(event.respondWith(input), f.result);
  f.dispatch(event);
  event.completion.resolve();
  assert.equal(await event.nativeResponse, response);
  await tick();
  assert.equal(calls, 1);
  assert.equal(event.response, input);
  assert.equal(last(f.records, "respondWith-unavailable").pendingResponses, 0);
  const direct = new f.FetchEvent();
  assert.equal(direct.respondWith(response), f.result);
  f.dispatch(direct);
  direct.completion.resolve();
  await tick();
  assert.equal(direct.response, response);
  assert.equal(response.bodyUsed, false);
  assert.equal(last(f.records, "respondWith-unavailable").pendingResponses, 0);
});

test("two deferred responses and browser handling settle independently in reverse order", async () => {
  const f = fixture();
  const first = new f.FetchEvent();
  const second = new f.FetchEvent();
  const a = deferred(),
    b = deferred();
  assert.equal(first.respondWith(a.promise), f.result);
  f.dispatch(first);
  assert.equal(second.respondWith(b.promise), f.result);
  f.dispatch(second);
  assert.equal(first.response, a.promise);
  assert.equal(last(f.records, "respondWith-request").pendingResponses, 2);
  assert.equal(last(f.records, "fetch-event").pendingHandled, 2);
  b.resolve(new Response("second"));
  await tick();
  assert.equal(last(f.records, "respondWith-fulfilled").pendingResponses, 1);
  assert.equal(last(f.records, "respondWith-fulfilled").pendingHandled, 2);
  second.completion.resolve();
  a.resolve(new Response("first"));
  first.completion.resolve();
  await tick();
  assert.equal(last(f.records, "handled-fulfilled").pendingHandled, 0);
  assert.equal(last(f.records, "respondWith-fulfilled").pendingResponses, 0);
  const ids = f.records
    .filter((r) => r.operation === "respondWith-request")
    .map((r) => r.eventId);
  assert.equal(new Set(ids).size, 2);
  assert.equal(last(f.records, "respondWith-fulfilled").eventId, ids[0]);
});

test("rejected responses and handled keep their original failures without logging secrets", async () => {
  const f = fixture();
  const event = new f.FetchEvent();
  const response = deferred();
  event.respondWith(response.promise);
  f.dispatch(event);
  response.reject(f.failure);
  event.completion.reject(f.failure);
  await assert.rejects(event.response, (error) => error === f.failure);
  await assert.rejects(event.handled, (error) => error === f.failure);
  await tick();
  assert.equal(last(f.records, "respondWith-rejected").pendingResponses, 0);
  assert.equal(last(f.records, "handled-rejected").pendingHandled, 0);
  assert.doesNotMatch(
    JSON.stringify(f.records),
    /private|token|credential|cookie|must-not-be-logged/,
  );
});

test("observing a Response never reads, clones, or locks its streaming body", async () => {
  const f = fixture();
  let writer;
  const body = new ReadableStream({
    start(controller) {
      writer = controller;
    },
  });
  const response = new Response(body);
  response.clone = () => {
    throw Error("cloning is not observation");
  };
  const input = Promise.resolve(response);
  const event = new f.FetchEvent();
  event.respondWith(input);
  f.dispatch(event);
  event.completion.resolve();
  await tick();
  assert.equal(last(f.records, "respondWith-fulfilled").pendingResponses, 0);
  assert.equal(response.bodyUsed, false);
  assert.equal(body.locked, false);
  assert.equal(event.response, input);
  writer.enqueue(new TextEncoder().encode("original-stream"));
  writer.close();
  assert.equal(await (await event.response).text(), "original-stream");
});

test("native synchronous respondWith exceptions stay identical and never leave phantom pending", async () => {
  const f = fixture();
  const invalid = new f.FetchEvent();
  invalid.invalid = true;
  assert.throws(
    () => invalid.respondWith(Promise.resolve(new Response())),
    (error) => error === f.failure,
  );
  assert.equal(last(f.records, "respondWith-threw").pendingResponses, 0);
  await f.self.skipWaiting();
  assert.equal(last(f.records, "skipWaiting-request").pendingResponses, 0);
});

test("waitUntil is independent of a fulfilled response and preserves native return and throw", async () => {
  const f = fixture();
  const event = new f.FetchEvent();
  const lifetime = deferred();
  assert.equal(event.waitUntil(lifetime.promise), f.result);
  assert.equal(event.lifetimes[0], lifetime.promise);
  event.respondWith(Promise.resolve(new Response()));
  f.dispatch(event);
  event.completion.resolve();
  await tick();
  assert.equal(last(f.records, "respondWith-fulfilled").pending, 1);
  const invalid = new f.ExtendableEvent("message");
  invalid.invalid = true;
  assert.throws(
    () => invalid.waitUntil(Promise.resolve()),
    (error) => error === f.failure,
  );
  assert.equal(last(f.records, "waitUntil-threw").pending, 1);
  lifetime.resolve();
  await tick();
  assert.equal(last(f.records, "waitUntil-settled").pending, 0);
});

test("unhandled requests and unknown request metadata use closed records and monotonic times", async () => {
  const f = fixture();
  const event = new f.FetchEvent();
  event.request.method = "private-method";
  event.request.mode = "private-mode";
  event.request.destination = "private-destination";
  f.dispatch(event);
  event.completion.resolve();
  new f.ExtendableEvent("private-event").waitUntil(Promise.resolve());
  await tick();
  const row = last(f.records, "fetch-event");
  assert.equal(row.method, "other");
  assert.equal(row.bootAt, 1000);
  assert.equal(row.mode, "other");
  assert.equal(row.destination, "other");
  assert.equal(last(f.records, "handled-fulfilled").pendingHandled, 0);
  assert.equal(last(f.records, "waitUntil-request").event, "other");
  for (let i = 1; i < f.records.length; i++)
    assert.ok(f.records[i].elapsed_ms >= f.records[i - 1].elapsed_ms);
  assert.doesNotMatch(JSON.stringify(f.records), /private|token|cookie/);
});

test("missing handled support is explicit without phantom browser pending", async () => {
  const f = fixture();
  const event = new f.FetchEvent();
  event.handled = undefined;
  f.dispatch(event);
  f.dispatch(event);
  assert.equal(last(f.records, "handled-unavailable").pendingHandled, 0);
  assert.equal(
    f.records.filter((row) => row.operation === "fetch-event").length,
    1,
  );
});
