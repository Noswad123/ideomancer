//! Integrated admitted-lane projection checks; no header-call admission claim.
use super::*;

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn projection_batch_cross_family_facts_warnings_and_persistent_owners() {
    let python = oracle();
    let (_, scan) = capture(&[(
        "unit.py",
        br#"from arbitrary import eval as probe
def repeat(arg: Type=default) -> Result:
    outer(key=probe(), *star(), later=last(), **unpack())
    {key1(): value1(), **mapping(), key2(): value2()}
    body() if condition() else alternate()
    lambda a=pos1(), b=pos2(), *, c=kw1(), d=kw2(): probe()
    [element() for receiver()[slot()] in iterable() if guard()]
    {key(): value() for receiver()[slot()] in iterable() if guard()}
    f'{body() if test() else alternate():{width()}.{precision()}}'
    from nested import exec as probe
    def inner():
        from safe import read as probe
        probe()
    probe()
    # This trailing comment is not the definition's end line.
class repeat:
    probe()
    from safe import read as probe
    def method():
        probe()
def repeat():
    probe()
probe()
"#,
    )]);
    let native = assert_warning_candidates_match_ast(&python, &scan);
    assert_eq!(
        native["identity"]["policy"],
        "python-native-private-facts-15"
    );
    assert_eq!(native["diagnostics"].as_array().unwrap().len(), 5);
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn projection_batch_render_caps_replay_and_no_execution() {
    use super::python_native::extract_fact_candidates;
    let python = oracle();
    let (marker_root, _) = capture(&[("empty.py", b"")]);
    let marker = marker_root.path().join("not_executed");
    let arguments = (0..100)
        .map(|i| format!("hidden{i}()"))
        .collect::<Vec<_>>()
        .join(", ");
    let text = format!("from arbitrary import eval as probe\ndef f():\n    outer([{arguments}], payload=({{key(): probe()}} if test() else alternate()))\n    open({:?}, 'w').write('redacted')\n", marker.to_str().unwrap());
    let (root, scan) = capture(&[("unit.py", text.as_bytes())]);
    let first = assert_warning_candidates_match_ast(&python, &scan);
    assert_eq!(
        first["facts"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|fact| fact["kind"] == "call")
            .count(),
        107
    );
    fs::remove_file(root.path().join("unit.py")).unwrap();
    let replay = serde_json::to_value(extract_fact_candidates(&scan.sources).unwrap()).unwrap();
    assert_eq!(first["facts"], replay["facts"]);
    assert_eq!(first["diagnostics"], replay["diagnostics"]);
    assert!(!marker.exists());
}

#[test]
#[ignore = "requires explicitly selected trusted Python >=3.12 oracle"]
fn projection_batch_header_and_constructor_guards_stay_transactional() {
    use super::python_native::{extract_fact_candidates, CandidateStatus};
    let python = oracle();
    for suffix in [
        "@decorate()\ndef f(): pass\n",
        "def f(arg=hidden()): pass\n",
        "def f() -> hidden(): pass\n",
        "class C(hidden()): pass\n",
        "class C(metaclass=hidden()): pass\n",
        "from external import APIRouter as factory\n",
        "from external import FastAPI as factory\n",
    ] {
        let text = format!("from arbitrary import eval as probe\nprobe()\n{suffix}");
        let (_, scan) = capture(&[("unit.py", text.as_bytes())]);
        assert!(extract_captured(&python, &scan.sources).unwrap().complete);
        let native = extract_fact_candidates(&scan.sources).unwrap();
        assert_eq!(native.files[0].status, CandidateStatus::UnsupportedSyntax);
        assert_eq!(native.facts.len(), 1);
        assert!(native.diagnostics.is_empty());
    }
}
