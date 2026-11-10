//! Shared offline document shell. Callers own and escape the trusted body markup;
//! diagrams remain external images, never executable inline SVG.
use sha2::{Digest, Sha256};

const STYLE: &str = include_str!("viewer.css");
const SCRIPT: &str = include_str!("viewer.js");

/// Wrap renderer-owned, already-escaped markup in the shared offline viewer.
/// `body` is trusted HTML, not arbitrary user input. `title` is plain text.
pub fn document(title: &str, body: &str) -> String {
    let hash = base64(&Sha256::digest(SCRIPT.as_bytes()));
    format!(
        "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\">\n\
         <meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; img-src 'self'; script-src 'sha256-{hash}'; style-src 'unsafe-inline'; connect-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none'\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>{}</title><style>{STYLE}</style></head><body>{body}<script>{SCRIPT}</script></body></html>\n",
        escape(title)
    )
}

/// A single fixed local image with controls; all legend/detail markup stays outside.
pub fn diagram(alt: &str, caption: &str) -> String {
    diagram_for(alt, caption, "diagram.svg", "viewer").expect("fixed viewer identifiers")
}

/// Generated flat image names and unique viewer IDs only; never source paths or URLs.
pub fn diagram_for(
    alt: &str,
    caption: &str,
    svg_name: &str,
    viewer_id: &str,
) -> Result<String, String> {
    let safe = |s: &str| {
        !s.is_empty()
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
    };
    if !svg_name.strip_suffix(".svg").is_some_and(safe)
        || !safe(viewer_id)
        || !viewer_id.as_bytes()[0].is_ascii_alphabetic()
    {
        return Err("viewer requires a safe flat SVG filename and generated viewer ID".into());
    }
    Ok(format!(
        r#"<figure class="diagram-viewer" data-viewer data-viewer-state="loading">
<div class="viewer-toolbar" data-viewer-toolbar role="group" aria-label="Diagram controls">
<button type="button" data-viewer-action="zoom-in" disabled aria-label="Zoom in">+</button>
<button type="button" data-viewer-action="zoom-out" disabled aria-label="Zoom out">-</button>
<button type="button" data-viewer-action="fit" disabled>Fit</button>
<button type="button" data-viewer-action="reset" disabled>Reset 100%</button>
<button type="button" data-viewer-action="fullscreen" disabled hidden>Fullscreen</button>
<span data-viewer-ratio aria-label="Zoom ratio">100%</span>
</div>
<p class="viewer-help" id="{viewer_id}-help">Focus the diagram: + / - to zoom, arrows to pan, F to fit, 0 to reset. Drag with the left mouse button or scroll to pan. Ctrl/Meta + wheel zooms at the pointer. Touch uses native scrolling.</p>
<div class="viewer-viewport" data-viewer-viewport tabindex="0" role="region" aria-label="Diagram viewport" aria-describedby="{viewer_id}-help">
<div class="viewer-stage" data-viewer-stage><img data-viewer-image src="{svg_name}" alt="{}" draggable="false"></div>
</div>
<p class="viewer-status" data-viewer-status role="status" aria-live="polite" aria-atomic="true">Diagram controls become available when the image loads. If it is unavailable, read the details below.</p>
<noscript><p>JavaScript is disabled. Scroll the diagram at its original size, or <a href="{svg_name}">open the SVG</a>. Full text and details remain below.</p></noscript>
<figcaption>{}</figcaption>
</figure>"#,
        escape(alt),
        escape(caption)
    ))
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
        .replace('\r', "&#13;")
}

fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let bits = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        for i in 0..4 {
            out.push(if i > chunk.len() {
                '='
            } else {
                ALPHABET[((bits >> (18 - 6 * i)) & 63) as usize] as char
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_vectors() {
        for (plain, encoded) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(base64(plain.as_bytes()), encoded);
        }
        assert_eq!(base64(&[251, 255, 255]), "+///");
        assert_eq!(
            base64(&Sha256::digest(b"abc")),
            "ungWv48Bz+pBQUDeXa4iI7ADYaOWF3qctBD/YfIAFa0="
        );
    }

    #[test]
    fn deterministic_shell_hashes_exactly_one_controller() {
        let body = diagram("Diagram", "Caption");
        let html = document("Title", &body);
        assert_eq!(html, document("Title", &body));
        assert_eq!(html.matches("<script>").count(), 1);
        let embedded = html
            .split_once("<script>")
            .unwrap()
            .1
            .split_once("</script>")
            .unwrap()
            .0;
        assert_eq!(embedded, SCRIPT);
        assert!(html.contains(&format!(
            "script-src 'sha256-{}';",
            base64(&Sha256::digest(embedded.as_bytes()))
        )));
        for policy in [
            "default-src 'none'",
            "img-src 'self';",
            "connect-src 'none'",
            "object-src 'none'",
            "base-uri 'none'",
            "form-action 'none'",
        ] {
            assert!(html.contains(policy));
        }
        assert!(!html.contains("script-src 'unsafe-inline'"));
        assert!(!html.contains("unsafe-eval"));
        assert!(!SCRIPT.contains("</script"));
    }

    #[test]
    fn plain_text_cannot_inject_svg_scripts_attributes_or_links() {
        let attack =
            "\"'><svg onload='bad()'></svg><script>bad()</script><a href='https://evil.invalid'> &";
        let body = diagram(attack, attack);
        let html = document(attack, &body);
        assert!(html.contains(&format!("<title>{}</title>", escape(attack))));
        assert!(body.contains(&format!("alt=\"{}\"", escape(attack))));
        assert!(body.contains(&format!("<figcaption>{}</figcaption>", escape(attack))));
        assert!(!html.contains("<svg"));
        assert_eq!(html.matches("<script>").count(), 1);
        assert_eq!(body.matches("<img ").count(), 1);
        assert_eq!(body.matches("src=\"").count(), 1);
        assert!(body.contains("src=\"diagram.svg\""));
        for link in body.split("href=\"").skip(1) {
            assert_eq!(link.split('"').next().unwrap(), "diagram.svg");
        }
        let trusted = "<nav><a href=\"legend.json\">Legend</a></nav><article id=\"details\">Escaped text</article>";
        assert!(document("Title", trusted).contains(trusted));
    }

    #[test]
    fn multiple_viewers_have_independent_images_and_help_ids() {
        let a = diagram_for("First", "Caption", "group-0001.svg", "group-0001").unwrap();
        let b = diagram_for("Second", "Caption", "group-0002.svg", "group-0002").unwrap();
        let html = document("Multiple", &(a + &b));
        for id in ["group-0001", "group-0002"] {
            assert_eq!(html.matches(&format!("id=\"{id}-help\"")).count(), 1);
            assert!(html.contains(&format!("aria-describedby=\"{id}-help\"")));
            assert!(html.contains(&format!("src=\"{id}.svg\"")));
        }
        assert!(SCRIPT.contains("querySelectorAll(\"[data-viewer]\").forEach"));
        assert_eq!(html.matches("<script>").count(), 1);
        for bad in [
            "../bad.svg",
            "a/b.svg",
            "https://bad.svg",
            "a.svg?x",
            "a\".svg",
            "a.svg.svg",
            ".svg",
        ] {
            assert!(diagram_for("", "", bad, "good").is_err());
        }
        for bad in ["", "../bad", "1bad", "bad\"", "bad id"] {
            assert!(diagram_for("", "", "good.svg", bad).is_err());
        }
    }

    /// Opt-in browser contract shared by every renderer using this document/diagram shell.
    #[test]
    #[ignore = "requires explicit IDEOMANCER_PUPPETEER_MODULE and already-installed Chromium"]
    fn offline_chromium_regression() {
        use std::{fs, path::PathBuf, process::Command};

        let module = PathBuf::from(std::env::var_os("IDEOMANCER_PUPPETEER_MODULE").expect(
            "set IDEOMANCER_PUPPETEER_MODULE to an existing absolute Puppeteer module path",
        ));
        assert!(
            module.is_absolute() && module.exists(),
            "invalid Puppeteer module path"
        );
        let fixtures = tempfile::tempdir().expect("temporary browser fixtures");
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="4000" height="1800" viewBox="0 0 4000 1800"><rect width="4000" height="1800" fill="white"/><rect x="20" y="20" width="3960" height="1760" fill="none" stroke="black"/><text x="80" y="120" font-size="48">Readable original-size offline diagram</text></svg>"#;
        let hostile_svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="4000" height="1800"><rect width="4000" height="1800" fill="white"/><text x="80" y="120">Hostile SVG stays an image</text><script>window.__viewerAttack=true;parent.__viewerAttack=true;parent.document.body.insertAdjacentHTML('beforeend','&lt;p id="svg-injected"&gt;bad&lt;/p&gt;');fetch('https://viewer-regression.invalid/script')</script><foreignObject width="400" height="200"><div xmlns="http://www.w3.org/1999/xhtml" id="foreign-payload"><img src="https://viewer-regression.invalid/foreign" onerror="parent.__viewerAttack=true"/></div></foreignObject><image href="https://viewer-regression.invalid/image" width="100" height="100"/></svg>"#;
        let responsive_svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100%" style="max-width: 12000px;" viewBox="0 0 12000 1800"><rect width="12000" height="1800" fill="white"/><text x="80" y="120" font-size="24">Responsive Mermaid coordinates must remain readable</text></svg>"#;
        let mut pages = Vec::new();
        for (name, image) in [
            ("ready", Some(svg)),
            ("missing", None),
            ("hostile", Some(hostile_svg)),
            ("responsive", Some(responsive_svg)),
        ] {
            let directory = fixtures.path().join(name);
            fs::create_dir(&directory).unwrap();
            if let Some(image) = image {
                let bytes = if name == "responsive" {
                    crate::render::draft::normalize_svg_dimensions(image.as_bytes().to_vec())
                        .unwrap()
                } else {
                    image.as_bytes().to_vec()
                };
                fs::write(directory.join("diagram.svg"), bytes).unwrap();
            }
            let mut body = diagram("Offline diagram", "Full text details remain available.");
            if name == "hostile" {
                body.push_str(r#"<button id="inline-handler-probe" onclick="window.__viewerAttack=true">CSP probe</button><script>window.__viewerAttack=true;document.body.insertAdjacentHTML('beforeend','<p id="inline-injected">bad</p>')</script>"#);
            }
            let page = directory.join("index.html");
            fs::write(&page, document("Offline viewer regression", &body)).unwrap();
            pages.push(page);
        }
        // IDEOMANCER_NODE may be an absolute executable or a name resolved on PATH.
        let node = std::env::var_os("IDEOMANCER_NODE").unwrap_or_else(|| "node".into());
        let output = Command::new(&node)
            .arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/viewer_browser.cjs"))
            .args(&pages)
            .env("IDEOMANCER_PUPPETEER_MODULE", &module)
            .output()
            .expect("run Node for the offline Chromium regression");
        assert!(
            output.status.success(),
            "browser regression failed ({:?})\nstdout:\n{}\nstderr:\n{}",
            output.status.code(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        print!("{}", String::from_utf8_lossy(&output.stdout));

        let mut body = String::new();
        for (id, image) in [
            ("first", Some(svg)),
            ("second", Some(svg)),
            ("missing", None),
        ] {
            if let Some(image) = image {
                fs::write(fixtures.path().join(format!("{id}.svg")), image).unwrap();
            }
            body.push_str(
                &diagram_for(id, "Independent viewer", &format!("{id}.svg"), id).unwrap(),
            );
        }
        let page = fixtures.path().join("multiple.html");
        fs::write(&page, document("Multiple offline viewers", &body)).unwrap();
        let output = Command::new(node).arg("-e").arg(r#"
const assert = require('node:assert/strict');
const {pathToFileURL} = require('node:url');
(async () => {
  const browser = await require(process.argv[1]).launch({headless:true, args:['--no-sandbox']});
  try {
    const page = await browser.newPage();
    const http = [], errors = [];
    page.on('request', r => { if (/^https?:/.test(r.url())) http.push(r.url()); });
    page.on('pageerror', e => errors.push(String(e)));
    await page.goto(pathToFileURL(process.argv[2]).href);
    await page.waitForFunction(() => [...document.querySelectorAll('[data-viewer]')].every(v => v.dataset.viewerState !== 'loading'));
    const states = () => page.$$eval('[data-viewer]', vs => vs.map(v => [v.dataset.viewerState, v.dataset.viewerScale]));
    assert.deepEqual(await states(), [['ready','1'],['ready','1'],['error',null]]);
    await page.$eval('[data-viewer]', v => v.querySelector('[data-viewer-action="zoom-in"]').click());
    assert.deepEqual(await states(), [['ready','1.25'],['ready','1'],['error',null]]);
    await page.$$eval('[data-viewer]', vs => vs[1].querySelector('[data-viewer-action="zoom-out"]').click());
    assert.deepEqual(await states(), [['ready','1.25'],['ready','0.8'],['error',null]]);
    assert.equal(await page.$$eval('[data-viewer-viewport]', ps => new Set(ps.map(p => p.getAttribute('aria-describedby'))).size),3);
    await page.setJavaScriptEnabled(false);
    await page.reload();
    assert.equal(await page.$$eval('[data-viewer-image]', imgs => imgs.filter(i => i.naturalWidth > 0).length),2);
    assert.deepEqual(http,[]); assert.deepEqual(errors,[]);
    console.log('PASS independent multi-viewer controls, missing-image isolation, unique help IDs, no-JS images, no HTTP');
  } finally { await browser.close(); }
})().catch(e => { console.error(e); process.exitCode = 1; });
"#).arg(module).arg(page).output().expect("multi-viewer browser regression");
        assert!(
            output.status.success(),
            "multi-viewer regression failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        print!("{}", String::from_utf8_lossy(&output.stdout));
    }
}
