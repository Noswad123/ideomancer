use std::fs::{self, File};
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const SVG_LIMIT: u64 = 8 * 1024 * 1024;
const LOG_LIMIT: u64 = 64 * 1024;
const MISSING_MMDC: &str =
    "Draft source saved. Rerun analyze after installing mmdc, or use --no-render.";

/// Render already-generated draft Mermaid with an operator-trusted mmdc.
/// The returned SVG is not sanitized; browser/network policy belongs to mmdc.
/// Unix cleanup covers the owned process group, not helpers that detach into
/// another group/session. This is not a sandbox for malicious executables.
pub fn svg(source: &str, executable: &Path) -> Result<Vec<u8>, String> {
    render(source, executable, Duration::from_secs(60))
}

pub(crate) fn svg_with_timeout(
    source: &str,
    executable: &Path,
    timeout: Duration,
) -> Result<Vec<u8>, String> {
    if timeout.is_zero() {
        return Err("Class UML render budget exhausted".into());
    }
    render(source, executable, timeout.min(Duration::from_secs(60)))
}

fn render(source: &str, executable: &Path, timeout: Duration) -> Result<Vec<u8>, String> {
    let cwd = std::env::current_dir().map_err(|_| "Cannot resolve mmdc executable directory.")?;
    let executable = if executable.is_absolute() {
        executable.to_path_buf()
    } else if executable.components().count() > 1 {
        cwd.join(executable)
    } else {
        std::env::var_os("PATH")
            .into_iter()
            .flat_map(|paths| std::env::split_paths(&paths).collect::<Vec<_>>())
            .map(|directory| cwd.join(directory).join(executable))
            .find(|candidate| {
                #[cfg(unix)]
                use std::os::unix::fs::PermissionsExt;
                candidate.metadata().is_ok_and(|metadata| {
                    #[cfg(unix)]
                    let executable = metadata.permissions().mode() & 0o111 != 0;
                    #[cfg(not(unix))]
                    let executable = true;
                    metadata.is_file() && executable
                })
            })
            .ok_or(MISSING_MMDC)?
    };
    let directory = tempfile::tempdir().map_err(|_| "Cannot create draft render directory.")?;
    // Make every argument absolute even when TMPDIR itself is relative.
    let directory_path =
        fs::canonicalize(directory.path()).map_err(|_| "Cannot resolve draft render directory.")?;
    let input = directory_path.join("source.mmd");
    let output = directory_path.join("output.svg");
    let stdout_path = directory_path.join("stdout.log");
    let stderr_path = directory_path.join("stderr.log");
    fs::write(&input, source).map_err(|_| "Cannot write draft render source.")?;
    let stdout = File::create(&stdout_path).map_err(|_| "Cannot capture mmdc output.")?;
    let stderr = File::create(&stderr_path).map_err(|_| "Cannot capture mmdc output.")?;
    let started = Instant::now();
    let mut command = Command::new(&executable);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = command
        .arg("-i")
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .current_dir(&directory_path)
        .stdin(Stdio::null())
        .stdout(stdout)
        .stderr(stderr)
        .spawn()
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                MISSING_MMDC.to_string()
            } else {
                "Cannot start mmdc for draft rendering.".to_string()
            }
        })?;

    // Files, rather than pipes, keep inherited descendant handles from blocking
    // completion. Polling bounds runaway disk growth, not just captured memory.
    let result = (|| {
        loop {
            let status = child
                .try_wait()
                .map_err(|_| "Cannot wait for mmdc draft rendering.".to_string())?;
            for (path, limit) in [
                (&stdout_path, LOG_LIMIT),
                (&stderr_path, LOG_LIMIT),
                (&output, SVG_LIMIT),
            ] {
                match fs::symlink_metadata(path) {
                    Ok(metadata) if metadata.len() > limit => {
                        return Err("mmdc exceeded the draft render output limit.".to_string());
                    }
                    Ok(_) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(_) => return Err("Cannot inspect mmdc output.".to_string()),
                }
            }
            if started.elapsed() >= timeout {
                return Err("mmdc draft rendering timed out.".to_string());
            }
            if let Some(status) = status {
                if !status.success() {
                    return Err(format!("mmdc draft rendering failed ({status})."));
                }
                break;
            }
            std::thread::sleep(
                Duration::from_millis(20).min(timeout.saturating_sub(started.elapsed())),
            );
        }

        let metadata = fs::symlink_metadata(&output)
            .map_err(|_| "mmdc did not produce a draft SVG.".to_string())?;
        if !metadata.is_file() {
            return Err("mmdc draft SVG must be a regular, non-symlink file.".to_string());
        }
        let mut options = fs::OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            // Do not follow a replacement symlink or block on a replacement FIFO.
            options.custom_flags(
                (rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::NONBLOCK).bits() as i32,
            );
        }
        let file = options
            .open(&output)
            .map_err(|_| "Cannot read draft SVG.")?;
        if !file
            .metadata()
            .map_err(|_| "Cannot inspect draft SVG.")?
            .is_file()
        {
            return Err("mmdc draft SVG must be a regular file.".to_string());
        }
        let mut bytes = Vec::new();
        file.take(SVG_LIMIT + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "Cannot read draft SVG.")?;
        if bytes.len() as u64 > SVG_LIMIT {
            return Err("mmdc exceeded the draft SVG size limit.".to_string());
        }
        normalize_svg_dimensions(bytes)
    })();
    #[cfg(unix)]
    {
        use rustix::process::{kill_process_group, test_kill_process_group, Pid, Signal};
        // process_group(0) makes this PID the owned PGID, never our caller's
        // group. Escalate even if the leader exited but helpers remain alive.
        let group = Pid::from_child(&child);
        if kill_process_group(group, Signal::TERM).is_ok() {
            // Puppeteer handles TERM by closing its browser (which can own a
            // separate process group). Give that hook a bounded chance to run.
            let graceful = Instant::now();
            while graceful.elapsed() < Duration::from_secs(1)
                && test_kill_process_group(group).is_ok()
            {
                let _ = child.try_wait();
                std::thread::sleep(Duration::from_millis(20));
            }
            let _ = kill_process_group(group, Signal::KILL);
        }
    }
    // Also reap the main child after group cleanup. The direct kill is a
    // fallback if group signalling fails, and is the non-Unix cleanup path.
    if result.is_err() {
        let _ = child.kill();
    }
    let _ = child.wait();
    result
}

/// Give responsive mmdc SVGs intrinsic image dimensions in viewBox coordinates.
/// This only edits root width/height, not SVG content; it is not sanitization.
pub(crate) fn normalize_svg_dimensions(bytes: Vec<u8>) -> Result<Vec<u8>, String> {
    const ROOT_LIMIT: usize = 64 * 1024;
    const ATTRIBUTE_LIMIT: usize = 256;
    // At the viewer's 8x maximum this stays below browser layout limits.
    const MAX_DIMENSION: f64 = 1_000_000.0;
    const INVALID: &str = "Invalid or unsupported draft SVG root dimensions.";
    const MALFORMED: &str = "Malformed draft SVG root attributes.";
    if bytes.len() as u64 > SVG_LIMIT {
        return Err("mmdc exceeded the draft SVG size limit.".into());
    }
    let text = std::str::from_utf8(&bytes).map_err(|_| "mmdc output is not SVG.")?;
    let mut root = text.trim_start_matches('\u{feff}').trim_start();
    if root.starts_with("<?xml ")
        || root.starts_with("<?xml\n")
        || root.starts_with("<?xml\t")
        || root.starts_with("<?xml\r")
    {
        let end = root
            .find("?>")
            .filter(|end| *end < ROOT_LIMIT)
            .ok_or("Malformed draft SVG XML declaration.")?;
        root = root[end + 2..].trim_start();
    }
    if !root.strip_prefix("<svg").is_some_and(|rest| {
        rest.starts_with('>') || rest.starts_with('/') || rest.starts_with(char::is_whitespace)
    }) {
        return Err("mmdc output is not SVG.".into());
    }
    let start = text.len() - root.len();
    let end = (start + ROOT_LIMIT).min(bytes.len());
    let mut cursor = start + 4;
    let mut attributes = std::collections::BTreeMap::new();
    let insertion;
    loop {
        let before_space = cursor;
        while cursor < end && matches!(bytes[cursor], b' ' | b'\t' | b'\r' | b'\n') {
            cursor += 1;
        }
        if cursor >= end {
            return Err("Draft SVG root exceeds the 64 KiB limit or is unterminated.".into());
        }
        if bytes[cursor] == b'>' || bytes[cursor] == b'/' {
            insertion = cursor;
            if bytes[cursor] == b'/' {
                cursor += 1;
            }
            if cursor >= end || bytes[cursor] != b'>' {
                return Err(MALFORMED.into());
            }
            break;
        }
        if cursor == before_space || attributes.len() >= ATTRIBUTE_LIMIT {
            return Err("Malformed draft SVG root or attribute count exceeds 256.".into());
        }
        let name_start = cursor;
        if !matches!(bytes[cursor], b'a'..=b'z' | b'A'..=b'Z' | b'_' | b':') {
            return Err(MALFORMED.into());
        }
        cursor += 1;
        while cursor < end
            && (bytes[cursor].is_ascii_alphanumeric()
                || matches!(bytes[cursor], b'_' | b':' | b'-' | b'.'))
        {
            cursor += 1;
        }
        let name = &text[name_start..cursor];
        while cursor < end && matches!(bytes[cursor], b' ' | b'\t' | b'\r' | b'\n') {
            cursor += 1;
        }
        if cursor >= end || bytes[cursor] != b'=' {
            return Err(MALFORMED.into());
        }
        cursor += 1;
        while cursor < end && matches!(bytes[cursor], b' ' | b'\t' | b'\r' | b'\n') {
            cursor += 1;
        }
        if cursor >= end || !matches!(bytes[cursor], b'\'' | b'"') {
            return Err(MALFORMED.into());
        }
        let quote = bytes[cursor];
        cursor += 1;
        let value_start = cursor;
        while cursor < end && bytes[cursor] != quote {
            if bytes[cursor] == b'<'
                || (bytes[cursor] < 0x20 && !matches!(bytes[cursor], b'\t' | b'\r' | b'\n'))
            {
                return Err(MALFORMED.into());
            }
            cursor += 1;
        }
        if cursor >= end {
            return Err(MALFORMED.into());
        }
        if attributes.insert(name, value_start..cursor).is_some() {
            return Err(MALFORMED.into());
        }
        cursor += 1;
    }
    let number = |s: &str| -> Option<f64> {
        if s.is_empty()
            || !s
                .bytes()
                .all(|c| c.is_ascii_digit() || matches!(c, b'+' | b'-' | b'.' | b'e' | b'E'))
        {
            return None;
        }
        s.parse::<f64>().ok().filter(|n| n.is_finite())
    };
    let dimension =
        |s: &str| -> Option<f64> { number(s).filter(|n| *n > 0.0 && *n <= MAX_DIMENSION) };
    let mut viewbox = None;
    if let Some(range) = attributes.get("viewBox") {
        let value = &text[range.clone()];
        let mut values = Vec::new();
        for part in value.split(',') {
            if part.trim().is_empty() {
                return Err(INVALID.into());
            }
            for token in part.split_ascii_whitespace() {
                values.push(number(token).ok_or(INVALID)?);
            }
        }
        if values.len() != 4 || values[2..].iter().any(|n| *n <= 0.0 || *n > MAX_DIMENSION) {
            return Err(INVALID.into());
        }
        viewbox = Some((values[2], values[3]));
    }
    let mut pixels = Vec::new();
    for name in ["width", "height"] {
        let Some(range) = attributes.get(name) else {
            pixels.push(None);
            continue;
        };
        let value = text[range.clone()].trim();
        if let Some(n) = dimension(value.strip_suffix("px").unwrap_or(value)) {
            pixels.push(Some(n));
        } else if value == "auto"
            || [
                "%", "em", "rem", "ex", "ch", "vw", "vh", "vmin", "vmax", "cm", "mm", "in", "pt",
                "pc",
            ]
            .iter()
            .any(|unit| {
                value
                    .strip_suffix(unit)
                    .is_some_and(|n| dimension(n).is_some())
            })
        {
            pixels.push(None);
        } else {
            return Err(INVALID.into());
        }
    }
    if pixels.iter().all(Option::is_some) {
        return Ok(bytes);
    }
    let (width, height) = viewbox.ok_or("Responsive draft SVG requires a valid root viewBox.")?;
    let mut replacements = Vec::new();
    let mut additions = String::new();
    for (name, value) in [("width", width), ("height", height)] {
        if let Some(range) = attributes.get(name) {
            replacements.push((range.clone(), format!("{value}px")));
        } else {
            additions.push_str(&format!(" {name}=\"{value}px\""));
        }
    }
    replacements.push((insertion..insertion, additions));
    replacements.sort_by_key(|(range, _)| range.start);
    let mut normalized = text.to_owned();
    for (range, value) in replacements.into_iter().rev() {
        normalized.replace_range(range, &value);
    }
    if normalized.len() as u64 > SVG_LIMIT {
        return Err("Normalized draft SVG exceeds the SVG size limit.".into());
    }
    Ok(normalized.into_bytes())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;

    #[test]
    fn normalizes_responsive_mermaid_root_only_preserving_content() {
        for quote in ['\'', '"'] {
            let root = format!("\u{feff} <?xml version=\"1.0\"?>\n<svg xmlns={quote}http://www.w3.org/2000/svg{quote} width={quote}100%{quote} style={quote}max-width: 12000px;{quote} data-note={quote}a > b{quote} viewBox={quote}-8 -10 12000 1800{quote}>");
            let content =
                "<svg viewBox='0 0 2 3' width='100%'><script>inertInImage()</script></svg></svg>";
            let result = String::from_utf8(
                normalize_svg_dimensions(format!("{root}{content}").into_bytes()).unwrap(),
            )
            .unwrap();
            assert!(result.contains(&format!("width={quote}12000px{quote}")));
            assert!(result.contains(" height=\"1800px\""));
            assert!(result.contains(&format!("viewBox={quote}-8 -10 12000 1800{quote}")));
            assert!(result.ends_with(content));
            assert!(result.contains(&format!("data-note={quote}a > b{quote}")));
            assert_eq!(
                normalize_svg_dimensions(result.as_bytes().to_vec()).unwrap(),
                result.as_bytes()
            );
        }
        for attrs in [
            "",
            "width='100%' height='50%'",
            "width='32'",
            "height='20px'",
            "width='auto' height='2em'",
        ] {
            let result = String::from_utf8(
                normalize_svg_dimensions(
                    format!("<svg {attrs} viewBox='0, 0, 1.2e4, 1800'/>").into_bytes(),
                )
                .unwrap(),
            )
            .unwrap();
            assert!(result.contains("12000px"));
            assert!(result.contains("1800px"));
        }
        for svg in [
            "<svg width='640' height='480px' viewBox='0 0 12000 1800'/>",
            "<svg width='640px' height='480'/>",
        ] {
            assert_eq!(
                normalize_svg_dimensions(svg.as_bytes().to_vec()).unwrap(),
                svg.as_bytes()
            );
        }
    }

    #[test]
    fn rejects_invalid_geometry_and_malformed_root_without_nested_fallback() {
        for viewbox in [
            "0 0 0 20",
            "0 0 -1 20",
            "0 0 NaN 20",
            "0 0 inf 20",
            "0 0 1e309 20",
            "0 0 1000001 20",
            "0 0 20",
            "0 0 20 30 40",
            "0,,0,20,30",
            "0 0 20 30,",
            "0 0 &size; 30",
        ] {
            let svg = format!("<svg width='100%' viewBox='{viewbox}'/>");
            assert!(
                normalize_svg_dimensions(svg.into_bytes()).is_err(),
                "{viewbox}"
            );
        }
        for attrs in [
            "width='-1'",
            "height='0'",
            "width='NaN'",
            "width='infinity'",
            "width='1e300'",
            "height='oops'",
            "width='1000001px'",
            "width='-10%'",
            "width='1' width='2'",
            "viewBox='0 0 1 1' viewBox='0 0 2 2'",
            "width=100",
            "width '100'",
            "width\u{000b}='100'",
            "width=\u{000c}'100'",
            "data-note='bad\0value'",
            "width='100'height='20'",
            "data-a='unterminated",
            "width='100' / bad",
            "data-a='<svg'",
        ] {
            let svg = format!("<svg {attrs} viewBox='0 0 12000 1800'/>");
            assert!(
                normalize_svg_dimensions(svg.into_bytes()).is_err(),
                "{attrs}"
            );
        }
        for svg in [
            "<svg data-viewBox='0 0 12000 1800'><svg viewBox='0 0 20 30'/></svg>",
            "<svg><svg viewBox='0 0 12000 1800'/></svg>",
            "<svgish viewBox='0 0 1 1'/>",
            "<svg / >",
            "<svg width='1' height='1'",
            "<?xml broken<svg viewBox='0 0 1 1'/>",
        ] {
            assert!(
                normalize_svg_dimensions(svg.as_bytes().to_vec()).is_err(),
                "{svg}"
            );
        }
    }

    #[test]
    fn normalization_bounds_root_attributes_and_expanded_output() {
        let oversized_root = format!(
            "<svg data-note='{}' viewBox='0 0 1 1'/>",
            "x".repeat(64 * 1024)
        );
        assert!(normalize_svg_dimensions(oversized_root.into_bytes()).is_err());
        let attrs: String = (0..257).map(|i| format!(" a{i}='x'")).collect();
        assert!(
            normalize_svg_dimensions(format!("<svg{attrs} viewBox='0 0 1 1'/>").into_bytes())
                .is_err()
        );
        let mut svg = "<svg viewBox='0 0 12000 1800'>".to_owned();
        svg.push_str(&" ".repeat(SVG_LIMIT as usize - svg.len() - 6));
        svg.push_str("</svg>");
        assert_eq!(svg.len(), SVG_LIMIT as usize);
        assert!(normalize_svg_dimensions(svg.into_bytes())
            .unwrap_err()
            .contains("size limit"));
    }

    #[test]
    fn render_returns_normalized_responsive_geometry() {
        let directory = tempfile::tempdir().unwrap();
        let executable = stub(
            directory.path(),
            "printf '<svg width=\"100%%\" viewBox=\"0 0 12000 1800\"></svg>' > \"$4\"",
        );
        let result = String::from_utf8(svg("flowchart LR", &executable).unwrap()).unwrap();
        assert!(result.contains("width=\"12000px\""));
        assert!(result.contains("height=\"1800px\""));
    }

    fn stub(directory: &Path, body: &str) -> PathBuf {
        let executable = directory.join("mmdc");
        fs::write(&executable, format!("#!/bin/sh\nset -eu\n{body}\n")).unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
        executable
    }

    #[test]
    fn preserves_source_and_uses_isolated_absolute_paths() {
        let directory = tempfile::tempdir().unwrap();
        let source =
            "---\ntitle: \"Draft: \\\"quoted\\\" & <title>\"\n---\nflowchart TD\n A --> B\n";
        fs::write(directory.path().join("expected.mmd"), source).unwrap();
        let executable = stub(
            directory.path(),
            r#"
test "$#" = 4
test "$1" = -i
test "$3" = -o
case "$2" in /*/source.mmd) ;; *) exit 1 ;; esac
case "$4" in /*/output.svg) ;; *) exit 1 ;; esac
test "$(dirname "$2")" = "$PWD"
test "$(dirname "$4")" = "$PWD"
cmp "$2" "$(dirname "$0")/expected.mmd"
printf '<svg width="10" height="10"></svg>' > "$4"
"#,
        );
        assert_eq!(
            svg(source, &executable).unwrap(),
            b"<svg width=\"10\" height=\"10\"></svg>"
        );
    }

    #[test]
    fn accepts_declaration_and_whitespace() {
        let directory = tempfile::tempdir().unwrap();
        let executable = stub(
            directory.path(),
            "printf '  <?xml version=\"1.0\"?>\\n <svg width=\"10\" height=\"10\"></svg>' > \"$4\"",
        );
        assert!(svg("flowchart TD", &executable).is_ok());
    }

    #[test]
    fn missing_executable_is_actionable() {
        let directory = tempfile::tempdir().unwrap();
        let error = svg("secret source", &directory.path().join("missing")).unwrap_err();
        assert!(error.contains("source saved"));
        assert!(error.contains("installing mmdc"));
        assert!(error.contains("--no-render"));
        assert!(!error.contains("secret"));
    }

    #[test]
    fn rejects_failure_without_disclosing_diagnostics() {
        let directory = tempfile::tempdir().unwrap();
        let executable = stub(directory.path(), "printf 'secret diagnostic' >&2; exit 7");
        let error = svg("secret source", &executable).unwrap_err();
        assert!(error.contains('7'));
        assert!(!error.contains("secret"));
    }

    #[test]
    fn rejects_missing_nonregular_symlink_and_non_svg_outputs() {
        let directory = tempfile::tempdir().unwrap();
        for body in [
            "exit 0",
            "mkdir \"$4\"",
            "mkfifo \"$4\"",
            "printf '<svg></svg>' > other.svg; ln -s other.svg \"$4\"",
            "printf 'not SVG' > \"$4\"",
            "printf '<svgish></svgish>' > \"$4\"",
        ] {
            let executable = stub(directory.path(), body);
            assert!(svg("flowchart TD", &executable).is_err(), "{body}");
        }
    }

    #[test]
    fn rejects_oversized_svg_and_diagnostics() {
        let directory = tempfile::tempdir().unwrap();
        for body in [
            "dd if=/dev/zero of=\"$4\" bs=1048576 count=9 2>/dev/null",
            "dd if=/dev/zero bs=65536 count=2 2>/dev/null",
            "dd if=/dev/zero bs=65536 count=2 >&2 2>/dev/null",
        ] {
            let executable = stub(directory.path(), body);
            assert!(svg("flowchart TD", &executable)
                .unwrap_err()
                .contains("limit"));
        }
    }

    #[test]
    fn cleans_up_long_lived_same_group_descendants() {
        let directory = tempfile::tempdir().unwrap();
        // Include TERM-resistant helpers to exercise escalation, not just TERM.
        for resistant in [false, true] {
            for outcome in ["timeout", "limit", "failure", "success"] {
                let pid_path = directory.path().join("descendant.pid");
                let _ = fs::remove_file(&pid_path);
                let _ = fs::remove_file(directory.path().join("descendant.ready"));
                let action = match outcome {
                    "limit" => "dd if=/dev/zero bs=65536 count=2 2>/dev/null; wait \"$descendant\"",
                    "failure" => "exit 7",
                    "success" => "printf '<svg width=\"10\" height=\"10\"></svg>' > \"$4\"; exit 0",
                    _ => "wait \"$descendant\"",
                };
                let body = format!(
                    r#"
ready="$(dirname "$0")/descendant.ready"
({ignore_term} : > "$ready"; exec sleep 60) &
descendant=$!
printf '%s\n' "$descendant" > "$(dirname "$0")/descendant.pid"
trap 'wait "$descendant" || :; exit 0' TERM
while [ ! -f "$ready" ]; do sleep 0.01; done
{action}
"#,
                    ignore_term = if resistant { "trap '' TERM;" } else { "" },
                );
                let executable = stub(directory.path(), &body);
                let start = Instant::now();
                // Leave startup headroom when extraction/selection tests run in
                // parallel; still require the timeout to terminate the helper.
                let result = render("flowchart TD", &executable, Duration::from_secs(3));
                match outcome {
                    "success" => assert!(result.is_ok()),
                    "timeout" => assert!(result.unwrap_err().contains("timed out")),
                    "limit" => assert!(result.unwrap_err().contains("limit")),
                    _ => assert!(result.unwrap_err().contains('7')),
                }
                assert!(start.elapsed() < Duration::from_secs(6));
                let pid: u32 = fs::read_to_string(&pid_path)
                    .unwrap()
                    .trim()
                    .parse()
                    .unwrap();
                let deadline = Instant::now() + Duration::from_secs(2);
                loop {
                    // ps is portable across Linux/macOS. A killed orphan may
                    // briefly be a zombie until init reaps it; it is not alive.
                    let status = Command::new("ps")
                        .args(["-o", "stat=", "-p", &pid.to_string()])
                        .output()
                        .unwrap();
                    let state = String::from_utf8(status.stdout).unwrap();
                    if state.trim().is_empty() || state.trim_start().starts_with('Z') {
                        break;
                    }
                    if Instant::now() >= deadline {
                        // Do not leak a test helper if cleanup regresses.
                        let _ = rustix::process::kill_process(
                            rustix::process::Pid::from_raw(pid as i32).unwrap(),
                            rustix::process::Signal::KILL,
                        );
                        panic!("{outcome}: descendant {pid} is still running ({state})");
                    }
                    std::thread::sleep(Duration::from_millis(20));
                }
            }
        }
    }

    #[test]
    fn resolves_relative_executable_before_changing_directory() {
        let cwd = std::env::current_dir().unwrap();
        let directory = tempfile::tempdir_in(&cwd).unwrap();
        let executable = stub(
            directory.path(),
            "printf '<svg width=\"10\" height=\"10\"></svg>' > \"$4\"",
        );
        let relative = executable.strip_prefix(&cwd).unwrap();
        assert!(svg("flowchart TD", relative).is_ok());
    }
}
