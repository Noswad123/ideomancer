'use strict';

// Explicit opt-in with already installed tools; never downloads a browser.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { createHash } = require('node:crypto');
const { pathToFileURL } = require('node:url');

(async () => {
  const modulePath = process.env.IDEOMANCER_PUPPETEER_MODULE;
  assert(modulePath && path.isAbsolute(modulePath));
  const [viewer, legendPath, expectedArgument] = process.argv.slice(2);
  const expectedClasses = expectedArgument === undefined ? 88 : Number(expectedArgument);
  assert(Number.isSafeInteger(expectedClasses) && expectedClasses > 0);
  assert(path.isAbsolute(viewer) && path.isAbsolute(legendPath));
  const legend = JSON.parse(fs.readFileSync(legendPath, 'utf8'));
  assert.equal(legend.entries.length, expectedClasses);
  const homes = legend.groups.flatMap(g => g.homeClassIds);
  assert.equal(homes.length, expectedClasses);
  assert.deepEqual([...new Set(homes)].sort(), legend.entries.map(e => e.id).sort());
  const html = fs.readFileSync(viewer, 'utf8');
  const scripts = [...html.matchAll(/<script>([\s\S]*?)<\/script>/g)];
  assert.equal(scripts.length, 1, 'one static controller, not one script per diagram');
  assert(html.includes(`sha256-${createHash('sha256').update(scripts[0][1]).digest('base64')}`));
  for (const group of legend.groups) {
    assert(group.homeClassIds.length <= 12);
    assert(fs.readFileSync(path.join(path.dirname(viewer), `${group.id}.mmd`), 'utf8').startsWith('classDiagram\n'));
  }
  const browser = await require(modulePath).launch({
    headless: true,
    ...(process.env.IDEOMANCER_CHROMIUM_EXECUTABLE ? { executablePath: process.env.IDEOMANCER_CHROMIUM_EXECUTABLE } : {}),
  });
  const errors = [], remote = [];
  try {
    const page = await browser.newPage();
    page.on('pageerror', error => errors.push(String(error)));
    page.on('console', message => { if (message.type() === 'error') errors.push(message.text()); });
    page.on('dialog', async dialog => { errors.push(dialog.message()); await dialog.dismiss(); });
    await page.setRequestInterception(true);
    page.on('request', request => {
      if (/^https?:/i.test(request.url())) { remote.push(request.url()); void request.abort(); }
      else void request.continue();
    });
    const load = () => page.goto(pathToFileURL(viewer).href, { waitUntil: 'load' });
    const details = async () => {
      assert.deepEqual((await page.$$eval('details[data-class-id]', ns => ns.map(n => n.dataset.classId))).sort(), homes.slice().sort());
      for (const e of legend.entries) {
        const selector = `details[data-class-id="${e.id}"]`;
        await page.click(`${selector} > summary`);
        assert.equal(await page.$eval(selector, n => n.open), true);
        const visible = await page.$eval(selector, n => n.innerText);
        for (const m of [...e.methods, ...e.attributes]) assert(visible.includes(m.name), `${e.symbol}.${m.name}`);
      }
    };
    await load();
    assert.equal((await page.$$('[data-viewer]')).length, legend.groups.length);
    const states = () => page.$$eval('[data-viewer]', ns => ns.map(n => ({ state: n.dataset.viewerState, scale: Number(n.dataset.viewerScale) })));
    assert((await states()).every(s => s.state === 'ready'));
    for (let i = 0; i < legend.groups.length; i++) {
      const selector = `#${legend.groups[i].id} [data-viewer]`;
      const before = await states();
      await page.click(`${selector} [data-viewer-action="zoom-in"]`);
      const after = await states();
      assert.equal(after[i].scale, before[i].scale * 1.25);
      for (let j = 0; j < before.length; j++) if (j !== i) assert.deepEqual(after[j], before[j]);
      await page.click(`${selector} [data-viewer-action="zoom-out"]`);
      await page.click(`${selector} [data-viewer-action="fit"]`);
      assert((await states())[i].scale > 0);
      await page.click(`${selector} [data-viewer-action="reset"]`);
      assert.equal((await states())[i].scale, 1);
    }
    assert.deepEqual(errors, []);
    await page.setJavaScriptEnabled(false);
    await load(); await details();
    assert(await page.$$eval('[data-viewer-image]', ns => ns.every(n => n.complete && n.naturalWidth > 0)));
    assert.equal((await page.$$('iframe, object, svg')).length, 0);
    assert.equal((await page.$$('img')).length, legend.groups.length, 'hostile names must not inject images');
    await page.setJavaScriptEnabled(true);
    const svg = path.join(path.dirname(viewer), `${legend.groups[0].id}.svg`);
    const backup = `${svg}.browser-backup`;
    fs.renameSync(svg, backup);
    try {
      await page.setCacheEnabled(false); await load();
      const all = await states(); assert.equal(all[0].state, 'error'); assert(all.slice(1).every(s => s.state === 'ready'));
      assert(await page.$eval(`[id="${legend.groups[0].id}"] [data-viewer]`, n => [...n.querySelectorAll('button')].every(b => b.disabled)));
      await page.click(`#${legend.groups[1].id} [data-viewer-action="zoom-in"]`);
      assert.equal((await states())[1].scale, 1.25);
      await details();
      assert(errors.every(e => e.includes('ERR_FILE_NOT_FOUND')), errors.join('\n'));
    } finally { fs.renameSync(backup, svg); }
    errors.length = 0; await load(); assert((await states()).every(s => s.state === 'ready'));
    for (const viewport of [{ width: 390, height: 844 }, { width: 1440, height: 1000 }]) {
      await page.setViewport(viewport);
      assert(await page.$eval('details > summary', n => n.getBoundingClientRect().width > 0));
    }
    assert.deepEqual(remote, []); assert.deepEqual(errors, []);
    console.log(`${expectedClasses} UML homes, independent controls, JS-off details/images and isolated missing SVG verified`);
  } finally { await browser.close(); }
})().catch(error => { console.error(error); process.exitCode = 1; });
