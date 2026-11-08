import { chromium, expect } from '@playwright/test';
import { LOCALES } from '../../apps/web/src/services/locale.ts';
import { PUBLIC_DOCUMENTS } from '../../apps/web/src/services/seo/document.ts';
const base = process.argv[2];
if (!/^http:\/\/127\.0\.0\.1:\d+$/.test(base ?? '')) throw Error('Only the owned loopback fixture is accepted');
const browser = await chromium.launch();
try {
  const noScript = await browser.newContext({ javaScriptEnabled: false, viewport: { width: 360, height: 780 } });
  const page = await noScript.newPage();
  for (const locale of LOCALES) for (const path of PUBLIC_DOCUMENTS) {
    const response = await page.goto(`${base}/${locale}${path}`);
    expect(response.status()).toBe(200);
    await expect(page.locator('html')).toHaveAttribute('lang', locale);
    await expect(page.locator('h1')).toBeVisible();
    expect(await page.locator('link[rel=canonical]').getAttribute('href')).toBe(`https://sweeper.example.test/${locale}${path}`);
    expect(await page.locator('link[rel=alternate]').count()).toBe(9);
  }
  await noScript.close();
  const context = await browser.newContext({ viewport: { width: 360, height: 780 } });
  const active = await context.newPage();
  const errors = [];
  active.on('pageerror', error => errors.push(error.name));
  await active.goto(`${base}/ko/learn`);
  await active.getByRole('link', { name: '튜토리얼 시작', exact: true }).click();
  await expect(active.getByRole('gridcell')).toHaveCount(9);
  expect(errors).toEqual([]);
  const privateResponse = await active.goto(`${base}/ko/me/account?private=fixture`);
  expect(privateResponse.headers()['cache-control']).toBe('no-store');
  await expect(active.locator('meta[name=robots]')).toHaveAttribute('content', /noindex/);
  expect(await active.locator('link[rel=canonical]').count()).toBe(0);
  await context.close();
  console.log('Docker nginx: 32 mobile JavaScript-off documents, real WASM tutorial and private head PASS');
} finally {
  await browser.close();
}
