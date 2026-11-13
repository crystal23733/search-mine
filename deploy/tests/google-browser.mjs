// Synthetic SDK responses only. This verifies our integration, not Google's live service.
import {chromium,expect} from '@playwright/test';
import {syntheticGoogleConsent} from '../../tests/fixtures/google-consent.mjs';
const base=process.argv[2];
if(!/^http:\/\/127\.0\.0\.1:\d+$/.test(base??''))throw Error('Owned loopback fixture required');
const browser=await chromium.launch();
try {
  const context=await browser.newContext();
  let insertions=0,escaped=0;
  const denied=syntheticGoogleConsent(Math.floor(Date.now()/1000),{advertising:false,analytics:false,measurement:false});
  const allowed=syntheticGoogleConsent(Math.floor(Date.now()/1000));
  await context.route('**/*',async route=>{
    const url=new URL(route.request().url());
    if(url.origin===base)return route.continue();
    if(url.origin==='https://pagead2.googlesyndication.com' && url.pathname==='/pagead/js/adsbygoogle.js') {
      insertions++;
      return route.fulfill({contentType:'application/javascript',headers:{'access-control-allow-origin':'*'},body:`(() => {
        const callbacks=new Map();let id=0;let data=${JSON.stringify(denied)};
        const state=window.__googleFixture={pause:[window.adsbygoogle.pauseAdRequests],revocations:0,commands:[],choose(){data={...${JSON.stringify(allowed)},eventStatus:'useractioncomplete'};for(const [key,fn] of callbacks)fn({...data,listenerId:key},true);}};
        Object.defineProperty(window.adsbygoogle,'pauseAdRequests',{get(){return state.pause.at(-1);},set(value){state.pause.push(value);}});
        window.__tcfapi=(command,version,fn,parameter)=>{state.commands.push(command);if(command==='addEventListener'){callbacks.set(++id,fn);fn({...data,listenerId:id},true);}else if(command==='removeEventListener'){callbacks.delete(parameter);fn(true,true);}};
        const queue=[];queue.push=(item)=>{item.CONSENT_API_READY?.();return 1;};
        window.googlefc={callbackQueue:queue,showRevocationMessage(){state.revocations++;}};
      })();`});
    }
    escaped++;return route.abort();
  });
  const page=await context.newPage();
  const errors=[];page.on('pageerror',e=>errors.push(e.message));
  const response=await page.goto(`${base}/ko/rules`);
  expect(response.headers()['referrer-policy']).toBe('strict-origin');
  const csp=response.headers()['content-security-policy'];
  expect(csp).toContain("'strict-dynamic'");expect(csp).not.toMatch(/script-src[^;]*(?:unsafe-inline|unsafe-eval)/);
  await expect.poll(()=>page.evaluate(()=>window.__googleFixture?.pause)).toEqual([1]);
  expect(await page.locator('script[data-liar-auto-ads]').count()).toBe(1);
  const nonce=await page.locator('script[type=module]').evaluate(node=>node.nonce);
  expect(nonce).toMatch(/^[0-9a-f]{32}$/);expect(csp).toContain(`'nonce-${nonce}'`);
  await expect.poll(()=>page.evaluate(async()=>(await navigator.serviceWorker.getRegistration())?.active?.state)).toBe("activated");
  await page.evaluate(()=>window.__googleFixture.choose());
  await expect.poll(()=>page.evaluate(()=>window.__googleFixture.pause.at(-1))).toBe(0);
  // A management request must survive the active-document retirement boundary.
  await page.getByRole('link',{name:'동의 설정',exact:true}).click();
  await expect(page).toHaveURL(`${base}/ko/consent`);
  await page.getByRole('button',{name:'동의 선택 다시 열기',exact:true}).click();
  await expect.poll(()=>page.evaluate(()=>window.__googleFixture?.revocations)).toBe(1);
  expect(await page.evaluate(()=>window.__googleFixture.pause)).toEqual([1]);
  expect(await page.evaluate(()=>Boolean(navigator.serviceWorker.controller))).toBe(true);
  await page.getByRole('link',{name:'Liar Sweeper',exact:true}).first().click();
  // Tutorial/new-player routing must not publish a game in the prepared document.
  await expect(page).toHaveURL(`${base}/ko/tutorial?return=%2F`);
  expect(await page.locator('script[data-liar-auto-ads]').count()).toBe(0);
  expect(await page.evaluate(()=>window.__googleFixture)).toBeUndefined();
  const privateResponse=await page.goto(`${base}/ko/me/privacy`);
  expect(privateResponse.headers()['referrer-policy']).toBe('no-referrer');
  expect(privateResponse.headers()['content-security-policy']).not.toContain('strict-dynamic');
  expect(await page.locator('script[data-liar-auto-ads]').count()).toBe(0);
  const policy=await page.goto(`${base}/ko/privacy`);
  await expect(page.getByText('Synthetic operator for automated tests',{exact:false})).toBeVisible();
  expect(policy.headers()['referrer-policy']).toBe('no-referrer');
  const first=await context.request.get(`${base}/ko/rules`);
  const second=await context.request.get(`${base}/ko/rules`,{headers:{'if-modified-since':first.headers()['last-modified']??'Sat, 14 Nov 2026 00:00:00 GMT'}});
  expect(second.status()).toBe(200);expect(await second.text()).toContain(second.headers()['content-security-policy'].match(/nonce-([a-f0-9]+)/)[1]);
  expect(await (await context.request.get(`${base}/ads.txt`)).text()).toBe('google.com, pub-0000000000000001, DIRECT, f08c47fec0942fa0\n');
  expect(insertions).toBeGreaterThanOrEqual(2);expect(escaped).toBe(0);expect(errors).toEqual([]);
  await context.close();console.log('Synthetic Google integration, nonce CSP, pause, isolated documents and published policies PASS');
} finally {await browser.close();}
