(() => {
  "use strict";
  document.querySelectorAll("[data-viewer]").forEach(viewer => {
  const viewport = viewer.querySelector("[data-viewer-viewport]");
  const stage = viewer.querySelector("[data-viewer-stage]");
  const image = viewer.querySelector("[data-viewer-image]");
  const ratio = viewer.querySelector("[data-viewer-ratio]");
  const status = viewer.querySelector("[data-viewer-status]");
  const buttons = [...viewer.querySelectorAll("[data-viewer-action]")];
  const fullscreen = viewer.querySelector('[data-viewer-action="fullscreen"]');
  const MIN_SCALE = 0.001;
  const MAX_SCALE = 8;
  let scale = 1;
  let width = 0;
  let height = 0;
  let settled = false;
  let ready = false;
  let announcement;
  let drag = null;

  function announce(message) {
    clearTimeout(announcement);
    announcement = setTimeout(() => {
      if (status.textContent !== message) status.textContent = message;
    }, 250);
  }

  function zoom(next, x = viewport.clientWidth / 2, y = viewport.clientHeight / 2, origin = false) {
    if (!ready || !Number.isFinite(next)) return;
    next = Math.min(MAX_SCALE, Math.max(MIN_SCALE, next));
    const left = (viewport.scrollLeft + x) / scale;
    const top = (viewport.scrollTop + y) / scale;
    scale = next;
    // The stage supplies layout bounds; the absolute image cannot leave an
    // unscaled overflow footprint when zooming out.
    stage.style.width = `${width * scale}px`;
    stage.style.height = `${height * scale}px`;
    image.style.transform = `scale(${scale})`;
    viewport.scrollLeft = origin ? 0 : left * scale - x;
    viewport.scrollTop = origin ? 0 : top * scale - y;
    viewer.dataset.viewerScale = String(scale);
    const percent = `${Number((scale * 100).toFixed(1))}%`;
    ratio.textContent = percent;
    announce(`Zoom ${percent}.`);
  }

  function fit() {
    zoom(Math.min(viewport.clientWidth / width, viewport.clientHeight / height), 0, 0, true);
  }

  function initialize() {
    if (settled) return;
    settled = true;
    width = image.naturalWidth;
    height = image.naturalHeight;
    if (!(width > 0 && height > 0)) {
      viewer.dataset.viewerState = "error";
      status.textContent = `Diagram unavailable: ${image.getAttribute("src")} is missing or invalid. Full text and details remain below.`;
      return;
    }
    ready = true;
    image.style.width = `${width}px`;
    image.style.height = `${height}px`;
    viewer.dataset.viewerState = "ready";
    buttons.forEach(button => { button.disabled = false; });
    fullscreen.hidden = !(document.fullscreenEnabled && viewer.requestFullscreen);
    zoom(1, 0, 0, true);
  }

  image.addEventListener("load", initialize, { once: true });
  image.addEventListener("error", initialize, { once: true });
  if (image.complete) initialize();

  viewer.querySelector("[data-viewer-toolbar]").addEventListener("click", async event => {
    const button = event.target.closest("[data-viewer-action]");
    if (!button || button.disabled || !ready) return;
    switch (button.dataset.viewerAction) {
      case "zoom-in": zoom(scale * 1.25); break;
      case "zoom-out": zoom(scale / 1.25); break;
      case "fit": fit(); break;
      case "reset": zoom(1, 0, 0, true); break;
      case "fullscreen":
        try {
          if (document.fullscreenElement === viewer) await document.exitFullscreen();
          else if (viewer.requestFullscreen) await viewer.requestFullscreen();
          else announce("Fullscreen unavailable. The scrollable diagram remains usable here.");
        } catch (_) {
          announce("Fullscreen unavailable. The scrollable diagram remains usable here.");
        }
        break;
    }
  });

  viewer.addEventListener("fullscreenchange", () => {
    fullscreen.textContent = document.fullscreenElement === viewer ? "Exit fullscreen" : "Fullscreen";
    // Keep scale and stage dimensions unchanged on fullscreen and resize.
  });

  viewport.addEventListener("keydown", event => {
    if (!ready || event.target !== viewport || document.activeElement !== viewport ||
        event.ctrlKey || event.metaKey || event.altKey) return;
    switch (event.key) {
      case "+": case "=": zoom(scale * 1.25); break;
      case "-": case "_": zoom(scale / 1.25); break;
      case "f": case "F": fit(); break;
      case "0": zoom(1, 0, 0, true); break;
      case "ArrowLeft": viewport.scrollLeft -= 60; break;
      case "ArrowRight": viewport.scrollLeft += 60; break;
      case "ArrowUp": viewport.scrollTop -= 60; break;
      case "ArrowDown": viewport.scrollTop += 60; break;
      default: return;
    }
    event.preventDefault();
  });

  viewport.addEventListener("wheel", event => {
    if (!ready || !(event.ctrlKey || event.metaKey)) return;
    event.preventDefault();
    const rect = viewport.getBoundingClientRect();
    const units = event.deltaMode === 1 ? 16 : event.deltaMode === 2 ? viewport.clientHeight : 1;
    const delta = Math.max(-500, Math.min(500, event.deltaY * units));
    zoom(scale * Math.exp(-delta * 0.002),
      event.clientX - rect.left - viewport.clientLeft,
      event.clientY - rect.top - viewport.clientTop);
  }, { passive: false });

  viewport.addEventListener("pointerdown", event => {
    // Touch and pen retain native scrolling and browser gestures.
    if (!ready || event.pointerType !== "mouse" || event.button !== 0 ||
        event.ctrlKey || event.metaKey || event.altKey) return;
    viewport.focus({ preventScroll: true });
    viewport.setPointerCapture(event.pointerId);
    drag = { id: event.pointerId, x: event.clientX, y: event.clientY,
      left: viewport.scrollLeft, top: viewport.scrollTop };
    viewer.setAttribute("data-viewer-dragging", "");
    event.preventDefault();
  });
  viewport.addEventListener("pointermove", event => {
    if (!drag || drag.id !== event.pointerId) return;
    viewport.scrollLeft = drag.left - (event.clientX - drag.x);
    viewport.scrollTop = drag.top - (event.clientY - drag.y);
  });
  function stopDrag(event) {
    if (!drag || drag.id !== event.pointerId) return;
    drag = null;
    viewer.removeAttribute("data-viewer-dragging");
    if (viewport.hasPointerCapture(event.pointerId)) viewport.releasePointerCapture(event.pointerId);
  }
  viewport.addEventListener("pointerup", stopDrag);
  viewport.addEventListener("pointercancel", stopDrag);
  viewport.addEventListener("lostpointercapture", stopDrag);
  });
})();
