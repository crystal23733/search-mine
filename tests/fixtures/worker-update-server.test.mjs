import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createConnection } from 'node:net';
import { once } from 'node:events';
import { setTimeout as pause } from 'node:timers/promises';
import { createWorkerUpdateServer } from '../e2e/helpers/worker-update-server.ts';

test('fixture shutdown closes an HTTP request whose body never completes', async () => {
  const server=await createWorkerUpdateServer();
  const url=new URL(server.origin);
  const socket=createConnection({host:url.hostname,port:Number(url.port)});
  let closed;
  const deadline=new AbortController();
  try {
    await once(socket,'connect');
    const replied=once(socket,'data');
    socket.write('GET /en/settings HTTP/1.1\r\nHost: localhost\r\nContent-Length: 1000000\r\nConnection: keep-alive\r\n\r\nx');
    await replied;
    closed=server.close();
    const result=await Promise.race([closed.then(()=>true),pause(300,undefined,{signal:deadline.signal}).then(()=>false)]);
    assert.equal(result,true,'fixture close must release active incomplete transports');
  } finally {
    deadline.abort();
    socket.destroy();
    await (closed ?? server.close());
  }
});
