"use strict";

// Opt-in only: use an existing Puppeteer module and its already-installed Chrome.
// Core: node tests/viewer_browser.cjs READY.html MISSING.html HOSTILE.html RESPONSIVE.html
// Smoke: node tests/viewer_browser.cjs --smoke GENERATED.html [GENERATED.html ...]
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const { pathToFileURL } = require("node:url");
const { createHash } = require("node:crypto");

const viewer = "[data-viewer]";
const viewport = "[data-viewer-viewport]";
const action = name => `[data-viewer-action="${name}"]`;
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));
let browser;
let phase = "startup";
const watchdog = setTimeout(() => {
  console.error(`viewer browser regression timed out during ${phase}`);
  if (browser?.process()) browser.process().kill("SIGKILL");
  process.exit(1);
}, 90000);

async function metrics(page) {
  return page.$eval(viewer, v => {
    const p = v.querySelector("[data-viewer-viewport]");
    const i = v.querySelector("[data-viewer-image]");
    const s = v.querySelector("[data-viewer-stage]");
    return {
      scale: Number(v.dataset.viewerScale), state: v.dataset.viewerState,
      left: p.scrollLeft, top: p.scrollTop, width: p.clientWidth, height: p.clientHeight,
      scrollWidth: p.scrollWidth, scrollHeight: p.scrollHeight,
      naturalWidth: i.naturalWidth, naturalHeight: i.naturalHeight,
      imageWidth: i.getBoundingClientRect().width,
      imageHeight: i.getBoundingClientRect().height,
      stageWidth: s.getBoundingClientRect().width,
      stageHeight: s.getBoundingClientRect().height,
      ratio: v.querySelector("[data-viewer-ratio]").textContent.trim(),
    };
  });
}

function near(actual, expected, tolerance = 0.00001) {
  assert.ok(Number.isFinite(actual), `non-finite value: ${actual}`);
  assert.ok(Math.abs(actual - expected) <= tolerance,
    `expected ${actual} to be within ${tolerance} of ${expected}`);
}

async function scaleIs(page, expected) {
  const m = await metrics(page);
  near(m.scale, expected);
  near(m.imageWidth, m.naturalWidth * expected, 1);
  assert.equal(m.ratio, `${Number((expected * 100).toFixed(1))}%`);
  return m;
}

async function main() {
  const modulePath = process.env.IDEOMANCER_PUPPETEER_MODULE;
  assert.ok(modulePath && path.isAbsolute(modulePath) && fs.existsSync(modulePath),
    "IDEOMANCER_PUPPETEER_MODULE must explicitly name an existing absolute Puppeteer module path");
  const puppeteer = require(modulePath);
  const args = process.argv.slice(2);
  const smoke = args[0] === "--smoke";
  if (smoke) args.shift();
  if (args[0] === "--") args.shift();
  assert.ok(smoke ? args.length > 0 : args.length === 4,
    "usage: viewer_browser.cjs READY.html MISSING.html HOSTILE.html RESPONSIVE.html | --smoke HTML ...");
  for (const file of args) assert.ok(fs.statSync(file).isFile(), `not a file: ${file}`);
  const executablePath = puppeteer.executablePath();
  assert.ok(fs.existsSync(executablePath), `Chrome must already be installed: ${executablePath}`);
  browser = await puppeteer.launch({ executablePath, headless: true });
  const network = [];
  const errors = [];

  async function open(file, javascript = true, staleGlobals = false) {
    phase = `${javascript ? "JavaScript" : "no-JavaScript"}: ${file}`;
    const page = await browser.newPage();
    page.setDefaultTimeout(7000);
    page.setDefaultNavigationTimeout(10000);
    await page.setViewport({ width: 1280, height: 900 });
    await page.setJavaScriptEnabled(javascript);
    await page.setRequestInterception(true);
    page.on("request", request => {
      if (/^https?:/i.test(request.url())) {
        network.push(request.url());
        void request.abort().catch(() => {});
      } else {
        void request.continue().catch(() => {});
      }
    });
    page.on("pageerror", error => errors.push(`${file}: ${error.message}`));
    if (staleGlobals) {
      await page.evaluateOnNewDocument(() => {
        window.zoom = window.fit = window.reset = () => { throw Error("stale global used"); };
        window.viewer = { scale: NaN, ready: false };
      });
    }
    await page.goto(pathToFileURL(path.resolve(file)).href, { waitUntil: "load" });
    return page;
  }

  async function ready(page) {
    await page.waitForSelector(`${viewer}[data-viewer-state="ready"]`);
    await scaleIs(page, 1);
    const dimensions = await metrics(page);
    assert.ok(dimensions.naturalWidth > 0 && dimensions.naturalHeight > 0,
      "generated SVG must have valid intrinsic dimensions");
    assert.equal(await page.$$eval(`${viewer} button:disabled`, nodes => nodes.length), 0);
    const policy = await page.$eval('meta[http-equiv="Content-Security-Policy"]', n => n.content);
    const scripts = await page.$$eval("script", nodes => nodes.map(n => n.textContent));
    const hashes = scripts.map(s => `'sha256-${createHash("sha256").update(s).digest("base64")}'`);
    const scriptPolicy = policy.match(/(?:^|;)\s*script-src\s+([^;]+)/);
    assert.ok(scriptPolicy, "missing script-src");
    assert.equal(scriptPolicy[1].trim().split(/\s+/).length, 1, "only controller hash allowed");
    assert.equal(hashes.filter(hash => scriptPolicy[1].trim() === hash).length, 1,
      "CSP must authorize exactly the controller's exact bytes");
    assert.ok(!/unsafe-eval|script-src[^;]*unsafe-inline/.test(policy));
    assert.match(policy, /connect-src 'none'/);
    assert.match(policy, /object-src 'none'/);
  }

  if (smoke) {
    for (const file of args) {
      const page = await open(file);
      await ready(page);
      await page.click(action("zoom-in"));
      await scaleIs(page, 1.25);
      await page.click(action("reset"));
      await scaleIs(page, 1);
      await page.close();
      console.log(`PASS smoke ${file}`);
    }
  } else {
    const page = await open(args[0], true, true);
    await ready(page);
    const initial = await metrics(page);
    assert.equal(initial.naturalWidth, 4000);
    assert.equal(initial.naturalHeight, 1800);
    assert.ok(initial.scrollWidth > initial.width && initial.scrollHeight > initial.height,
      "original-size diagram must overflow in both directions rather than shrink to illegibility");
    await page.click(action("zoom-in"));
    await scaleIs(page, 1.25);
    await page.click(action("zoom-out"));
    await scaleIs(page, 1);
    await page.click(action("fit"));
    const fitted = await metrics(page);
    await scaleIs(page, Math.min(fitted.width / 4000, fitted.height / 1800));
    assert.ok(fitted.scrollWidth <= fitted.width + 1 && fitted.scrollHeight <= fitted.height + 1);
    await page.click(action("reset"));
    let m = await scaleIs(page, 1);
    assert.equal(m.left, 0);
    assert.equal(m.top, 0);

    // Use interior coordinates and nonzero scroll so anchor checks cannot pass by clamping.
    await page.$eval(viewport, p => { p.scrollLeft = 700; p.scrollTop = 400; });
    const box = await page.$eval(viewport, p => {
      const r = p.getBoundingClientRect();
      return { x: r.left + p.clientLeft + 180, y: r.top + p.clientTop + 120 };
    });
    for (const modifier of ["ctrlKey", "metaKey"]) {
      const before = await metrics(page);
      await page.$eval(viewport, (p, { box, modifier }) => p.dispatchEvent(new WheelEvent("wheel", {
        bubbles: true, cancelable: true, clientX: box.x, clientY: box.y,
        deltaY: -80, [modifier]: true,
      })), { box, modifier });
      m = await scaleIs(page, before.scale * Math.exp(0.16));
      near((m.left + 180) / m.scale, (before.left + 180) / before.scale, 1.5);
      near((m.top + 120) / m.scale, (before.top + 120) / before.scale, 1.5);
    }
    const wheelScale = m.scale;
    await page.mouse.move(box.x, box.y);
    await page.mouse.wheel({ deltaY: 80 });
    await sleep(150);
    await scaleIs(page, wheelScale);
    await page.$eval(viewport, p => { p.scrollLeft = 700; p.scrollTop = 400; });
    const beforePan = await metrics(page);
    await page.mouse.move(box.x, box.y);
    await page.mouse.down();
    await page.mouse.move(box.x - 90, box.y - 60, { steps: 5 });
    await page.mouse.up();
    m = await metrics(page);
    near(m.left - beforePan.left, 90, 1);
    near(m.top - beforePan.top, 60, 1);
    assert.equal(await page.$eval(viewer, v => v.hasAttribute("data-viewer-dragging")), false);

    await page.click(action("reset")); // Toolbar focus must not activate viewport shortcuts.
    await page.keyboard.press("+");
    await scaleIs(page, 1);
    const toolbarFocus = await metrics(page);
    await page.keyboard.press("ArrowRight");
    await page.keyboard.press("ArrowDown");
    m = await metrics(page);
    assert.equal(m.left, toolbarFocus.left);
    assert.equal(m.top, toolbarFocus.top);
    await page.focus(viewport);
    await page.keyboard.press("+");
    await scaleIs(page, 1.25);
    await page.keyboard.press("-");
    await scaleIs(page, 1);
    await page.keyboard.press("ArrowRight");
    await page.keyboard.press("ArrowDown");
    m = await metrics(page);
    const beforeArrows = { left: m.left, top: m.top };
    await page.keyboard.press("ArrowLeft");
    await page.keyboard.press("ArrowUp");
    m = await metrics(page);
    near(beforeArrows.left - m.left, 60, 1);
    near(beforeArrows.top - m.top, 60, 1);
    await page.keyboard.press("f");
    m = await metrics(page);
    await scaleIs(page, Math.min(m.width / 4000, m.height / 1800));
    await page.keyboard.press("0");
    m = await scaleIs(page, 1);
    assert.equal(m.left, 0);
    assert.equal(m.top, 0);
    await page.click(action("zoom-in"));
    await page.setViewport({ width: 620, height: 760 });
    await sleep(100);
    await scaleIs(page, 1.25);

    phase = "fullscreen capability/transition";
    const fullscreenVisible = await page.$eval(action("fullscreen"), b => !b.hidden);
    if (fullscreenVisible) {
      await page.click(action("fullscreen"));
      await page.waitForFunction(() => document.fullscreenElement ||
        /fullscreen unavailable/i.test(document.querySelector("[data-viewer-status]").textContent));
      if (await page.evaluate(() => !!document.fullscreenElement)) {
        await page.waitForFunction(() => document.querySelector('[data-viewer-action="fullscreen"]').textContent === "Exit fullscreen");
        assert.equal(await page.$eval(action("fullscreen"), b => b.textContent), "Exit fullscreen");
        await scaleIs(page, 1.25);
        await page.click(action("fullscreen"));
        await page.waitForFunction(() => !document.fullscreenElement);
        await page.waitForFunction(() => document.querySelector('[data-viewer-action="fullscreen"]').textContent === "Fullscreen");
        assert.equal(await page.$eval(action("fullscreen"), b => b.textContent), "Fullscreen");
        await scaleIs(page, 1.25);
        console.log("PASS fullscreen enter/exit");
      } else console.log("SKIP fullscreen: browser denied the request");
    } else console.log("SKIP fullscreen: browser does not advertise support");
    await page.close();

    const missing = await open(args[1]);
    await missing.waitForSelector(`${viewer}[data-viewer-state="error"]`);
    assert.equal(await missing.$$eval(`${viewer} button:not(:disabled)`, n => n.length), 0);
    assert.match(await missing.$eval("[data-viewer-status]", n => n.textContent), /unavailable.*missing or invalid/i);
    await missing.close();

    const nojs = await open(args[0], false);
    m = await metrics(nojs);
    assert.equal(m.naturalWidth, 4000);
    near(m.imageWidth, 4000, 1);
    assert.ok(m.scrollWidth > m.width && m.scrollHeight > m.height);
    assert.equal(await nojs.$$eval(`${viewer} button:not(:disabled)`, n => n.length), 0);
    await nojs.$eval(viewport, p => { p.scrollLeft = 200; p.scrollTop = 100; });
    await nojs.$eval(action("zoom-in"), b => b.click());
    const unchanged = await metrics(nojs);
    assert.equal(unchanged.left, 200);
    assert.equal(unchanged.top, 100);
    near(unchanged.imageWidth, 4000, 1);
    assert.match(await nojs.$eval("noscript", n => n.textContent), /JavaScript is disabled/);
    await nojs.close();

    const hostile = await open(args[2], true, true);
    await ready(hostile);
    await hostile.click("#inline-handler-probe");
    await sleep(300);
    const safe = await hostile.evaluate(() => ({
      executed: !!window.__viewerAttack,
      added: document.querySelectorAll("#svg-injected, #inline-injected, #foreign-payload, svg, foreignObject, iframe, object, embed").length,
      images: document.querySelectorAll("img").length,
    }));
    assert.deepEqual(safe, { executed: false, added: 0, images: 1 },
      "SVG image context and CSP must keep SVG scripts, foreign HTML, and injected inline code inert");
    await hostile.click(action("zoom-in"));
    await scaleIs(hostile, 1.25);
    await hostile.close();
    console.log("PASS core viewer controls, geometry, keyboard, resize, missing SVG, no-JS, CSP, hostile SVG");

    const responsive = await open(args[3]);
    await ready(responsive);
    // These expectations come from the source viewBox, NOT naturalWidth: a
    // browser fallback of 300px must not define our supposed actual-size test.
    async function coordinateGeometry(scale) {
      const m = await scaleIs(responsive, scale);
      assert.equal(m.naturalWidth, 12000);
      assert.equal(m.naturalHeight, 1800);
      near(m.imageWidth, 12000 * scale, 1);
      near(m.imageHeight, 1800 * scale, 1);
      near(m.stageWidth, 12000 * scale, 1);
      near(m.stageHeight, 1800 * scale, 1);
      return m;
    }
    await coordinateGeometry(1);
    await responsive.click(action("fit"));
    let rm = await metrics(responsive);
    rm = await coordinateGeometry(Math.min(rm.width / 12000, rm.height / 1800));
    assert.ok(rm.scrollWidth <= rm.width + 1 && rm.scrollHeight <= rm.height + 1);
    await responsive.click(action("reset"));
    rm = await coordinateGeometry(1);
    assert.equal(rm.left, 0);
    assert.equal(rm.top, 0);
    await responsive.$eval(viewport, p => { p.scrollLeft = 4000; p.scrollTop = 400; });
    const anchor = await responsive.$eval(viewport, p => {
      const r = p.getBoundingClientRect();
      return { x: r.left + p.clientLeft + 200, y: r.top + p.clientTop + 100 };
    });
    const before = await metrics(responsive);
    await responsive.$eval(viewport, (p, a) => p.dispatchEvent(new WheelEvent("wheel", {
      bubbles: true, cancelable: true, ctrlKey: true,
      clientX: a.x, clientY: a.y, deltaY: -80,
    })), anchor);
    rm = await coordinateGeometry(Math.exp(.16));
    near((rm.left + 200) / rm.scale, before.left + 200, 1.5);
    near((rm.top + 100) / rm.scale, before.top + 100, 1.5);
    for (let i = 0; i < 12; i++) await responsive.click(action("zoom-in"));
    rm = await coordinateGeometry(8);
    near(rm.scrollWidth, 96000, 1);
    near(rm.scrollHeight, 14400, 1);
    await responsive.click(action("reset"));
    rm = await coordinateGeometry(1);
    assert.equal(rm.left, 0);
    assert.equal(rm.top, 0);
    await responsive.close();

    const responsiveNojs = await open(args[3], false);
    const sn = await metrics(responsiveNojs);
    assert.equal(sn.naturalWidth, 12000);
    assert.equal(sn.naturalHeight, 1800);
    near(sn.imageWidth, 12000, 1);
    near(sn.imageHeight, 1800, 1);
    assert.ok(sn.scrollWidth >= 12000 && sn.scrollHeight >= 1800);
    await responsiveNojs.$eval(viewport, p => { p.scrollLeft = 4000; p.scrollTop = 400; });
    const scrolled = await metrics(responsiveNojs);
    assert.equal(scrolled.left, 4000);
    assert.equal(scrolled.top, 400);
    await responsiveNojs.close();
    console.log("PASS responsive Mermaid 12000x1800 coordinate geometry, fit, anchor, 800%, reset, and no-JS");
  }
  assert.deepEqual(network, [], "offline viewer attempted HTTP(S) requests (blocked by test)");
  assert.deepEqual(errors, [], "unexpected uncaught browser errors");
  console.log("PASS no HTTP(S) attempts or uncaught browser errors");
}

main().catch(error => {
  console.error(`FAIL viewer browser regression during ${phase}:\n${error.stack || error}`);
  process.exitCode = 1;
}).finally(async () => {
  try { if (browser) await browser.close(); }
  finally { clearTimeout(watchdog); }
});
