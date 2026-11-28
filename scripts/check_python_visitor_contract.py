#!/usr/bin/env python3
"""Read-only finite G2/G3 inventory gate, not a helper/Ruff parity oracle.

Only the fixed repository JSON files are read. Only synthetic case strings are
parsed; no application modules, fixtures, helper code or Rust code are executed.
The compact hand-reviewed ASDL catalog below supplies types: _fields supplies
names/order, not types. This is an original inventory, not copied stdlib source.
"""

import ast
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parent.parent
PROFILE = "cpython-3.12.11-module-visitor-v1"
LIMIT = 2 * 1024 * 1024
TOTAL_LIMIT = 32 * 1024 * 1024
FILES = ("docs/python-visitor-contract.json", "docs/python-visitor-witnesses.json")
REQUIRED_IDS = (
    "g2_declarations", "g2_statements", "g2_expressions", "g2_patterns",
    "g2_markers", "g2_type_params", "g2_ignores", "g2_argument_order",
    "g2_comprehensions", "g2_optional_children", "g2_fstrings",
    "g2_interleaved_args", "g2_render_caps", "g3_metadata_order",
)
LEGACY = frozenset("Suite AugLoad AugStore Param slice Index ExtSlice Num Str Bytes NameConstant Ellipsis".split())
# suffix ? means optional; * means sequence (including optional Dict keys and
# arguments.kw_defaults elements). scalar never carries an AST child.
CATALOG = {
    "mod": """
Module body:stmt* type_ignores:type_ignore*
Interactive body:stmt*
Expression body:expr
FunctionType argtypes:expr* returns:expr
""",
    "stmt": """
FunctionDef name:identifier args:arguments body:stmt* decorator_list:expr* returns:expr? type_comment:string? type_params:type_param*
AsyncFunctionDef name:identifier args:arguments body:stmt* decorator_list:expr* returns:expr? type_comment:string? type_params:type_param*
ClassDef name:identifier bases:expr* keywords:keyword* body:stmt* decorator_list:expr* type_params:type_param*
Return value:expr?
Delete targets:expr*
Assign targets:expr* value:expr type_comment:string?
TypeAlias name:expr type_params:type_param* value:expr
AugAssign target:expr op:operator value:expr
AnnAssign target:expr annotation:expr value:expr? simple:int
For target:expr iter:expr body:stmt* orelse:stmt* type_comment:string?
AsyncFor target:expr iter:expr body:stmt* orelse:stmt* type_comment:string?
While test:expr body:stmt* orelse:stmt*
If test:expr body:stmt* orelse:stmt*
With items:withitem* body:stmt* type_comment:string?
AsyncWith items:withitem* body:stmt* type_comment:string?
Match subject:expr cases:match_case*
Raise exc:expr? cause:expr?
Try body:stmt* handlers:excepthandler* orelse:stmt* finalbody:stmt*
TryStar body:stmt* handlers:excepthandler* orelse:stmt* finalbody:stmt*
Assert test:expr msg:expr?
Import names:alias*
ImportFrom module:identifier? names:alias* level:int
Global names:identifier*
Nonlocal names:identifier*
Expr value:expr
Pass
Break
Continue
""",
    "expr": """
BoolOp op:boolop values:expr*
NamedExpr target:expr value:expr
BinOp left:expr op:operator right:expr
UnaryOp op:unaryop operand:expr
Lambda args:arguments body:expr
IfExp test:expr body:expr orelse:expr
Dict keys:expr?* values:expr*
Set elts:expr*
ListComp elt:expr generators:comprehension*
SetComp elt:expr generators:comprehension*
DictComp key:expr value:expr generators:comprehension*
GeneratorExp elt:expr generators:comprehension*
Await value:expr
Yield value:expr?
YieldFrom value:expr
Compare left:expr ops:cmpop* comparators:expr*
Call func:expr args:expr* keywords:keyword*
FormattedValue value:expr conversion:int format_spec:expr?
JoinedStr values:expr*
Constant value:constant kind:string?
Attribute value:expr attr:identifier ctx:expr_context
Subscript value:expr slice:expr ctx:expr_context
Starred value:expr ctx:expr_context
Name id:identifier ctx:expr_context
List elts:expr* ctx:expr_context
Tuple elts:expr* ctx:expr_context
Slice lower:expr? upper:expr? step:expr?
""",
    "expr_context": "Load\nStore\nDel",
    "boolop": "And\nOr",
    "operator": "Add\nSub\nMult\nMatMult\nDiv\nMod\nPow\nLShift\nRShift\nBitOr\nBitXor\nBitAnd\nFloorDiv",
    "unaryop": "Invert\nNot\nUAdd\nUSub",
    "cmpop": "Eq\nNotEq\nLt\nLtE\nGt\nGtE\nIs\nIsNot\nIn\nNotIn",
    "excepthandler": "ExceptHandler type:expr? name:identifier? body:stmt*",
    "pattern": """
MatchValue value:expr
MatchSingleton value:constant
MatchSequence patterns:pattern*
MatchMapping keys:expr* patterns:pattern* rest:identifier?
MatchClass cls:expr patterns:pattern* kwd_attrs:identifier* kwd_patterns:pattern*
MatchStar name:identifier?
MatchAs pattern:pattern? name:identifier?
MatchOr patterns:pattern*
""",
    "type_ignore": "TypeIgnore lineno:int tag:string",
    "type_param": "TypeVar name:identifier bound:expr?\nParamSpec name:identifier\nTypeVarTuple name:identifier",
    "support": """
comprehension target:expr iter:expr ifs:expr* is_async:int
arguments posonlyargs:arg* args:arg* vararg:arg? kwonlyargs:arg* kw_defaults:expr?* kwarg:arg? defaults:expr*
arg arg:identifier annotation:expr? type_comment:string?
keyword arg:identifier? value:expr
alias name:identifier asname:identifier?
withitem context_expr:expr optional_vars:expr?
match_case pattern:pattern guard:expr? body:stmt*
""",
}
MARKERS = frozenset(("expr_context", "boolop", "operator", "unaryop", "cmpop"))
SCALARS = frozenset(("identifier", "string", "int", "constant"))
SPAN = ["lineno", "col_offset", "end_lineno", "end_col_offset"]
SCHEMA_NOTES = [
    "field_rules[i] describes ordered_fields[i]; ? is optional, * is a sequence, ?* permits absent elements",
    "class witnesses name groups inherited by every node-bearing field; additional cases are included",
    "unique-call requires a descendant bare Call.func Name label occurring once in that case; structural proves population only",
    "visit includes generic child traversal and any metadata/render consumption; neutral scalars have no AST child obligation",
    "attributes are normalized span metadata, not unsupported generic classes; source/span compatibility remains G4",
]
OUTSIDE = frozenset(("Interactive", "Expression", "FunctionType"))
STRUCTURAL = frozenset((
    "Module.type_ignores", "Import.names", "ImportFrom.names",
    "NamedExpr.target", "TypeAlias.name", "MatchClass.cls",
))
METADATA = frozenset((
    "FunctionDef.name", "AsyncFunctionDef.name", "ClassDef.name",
    "Import.names", "ImportFrom.names", "ImportFrom.module", "ImportFrom.level",
    "alias.name", "alias.asname", "arg.arg", "keyword.arg", "Name.id",
    "Attribute.attr", "Constant.value", "BinOp.op",
))
HANDLERS = {
    "statements": ["src/analysis/python_ruff/facts.rs#Driver::statement_inner"],
    "expressions": ["src/analysis/python_ruff/expressions.rs#Driver::normalize", "src/analysis/python_ruff/facts.rs#Driver::visits"],
    "definitions": ["src/analysis/python_ruff/facts.rs#Driver::function", "src/analysis/python_ruff/facts.rs#Driver::class", "src/analysis/python_ruff/metadata.rs#Driver::decorators", "src/analysis/python_native/projection.rs#HeaderSchedule"],
    "imports": ["src/analysis/python_ruff/facts.rs#Driver::imports", "src/analysis/python_native/projection.rs#Projection"],
    "patterns": ["src/analysis/python_ruff/facts.rs#Driver::pattern"],
    "type_parameters": ["src/analysis/python_ruff/facts.rs#Driver::type_parameters"],
    "spans": ["src/analysis/python_ruff/type_comments.rs#CommentPlan::corrected_range", "src/analysis/python_ruff/facts.rs#Driver::span"],
    "outside": [],
}


def reviewed_inventory():
    """Independent field-type/disposition anchors; never infer types from cases."""
    result = []
    for category, text in CATALOG.items():
        for line in text.strip().splitlines():
            name, *tokens = line.split()
            outside = name in OUTSIDE
            handlers = ["statements"]
            if category == "expr" or category in MARKERS or name in ("comprehension", "keyword"):
                handlers = ["expressions"]
            if name in ("FunctionDef", "AsyncFunctionDef", "ClassDef", "arguments", "arg"):
                handlers = ["definitions"]
            if name in ("Import", "ImportFrom", "alias"):
                handlers = ["imports"]
            if category == "pattern" or name == "match_case":
                handlers = ["patterns"]
            if category == "type_param":
                handlers = ["type_parameters"]
            if category == "type_ignore":
                handlers = ["spans"]
            if outside:
                handlers = ["outside"]
            fields, rules = [], []
            for token in tokens:
                field, kind = token.split(":")
                fields.append(field)
                key = name + "." + field
                base = kind.rstrip("?*")
                if outside:
                    disposition, proof = "out-of-domain", "none"
                elif base in SCALARS:
                    disposition = "consume-metadata" if key in METADATA else "neutral-scalar"
                    proof = "none"
                elif base in MARKERS:
                    disposition = "consume-metadata" if key in METADATA else "neutral-marker"
                    proof = "structural"
                else:
                    disposition = "consume-metadata" if key in METADATA else "visit"
                    proof = "structural" if key in STRUCTURAL else "unique-call"
                rules.append([kind, disposition, proof])
            attributes = SPAN if category in ("stmt", "expr", "excepthandler", "pattern", "type_param") or name in ("arg", "keyword", "alias") else []
            result.append(dict(name=name, category=category,
                               domain="outside-Module" if outside else "Module",
                               rust_handlers=handlers, ordered_fields=fields,
                               field_rules=rules, attributes=attributes,
                               witnesses=[] if outside else ["module"]))
    return result


def require(condition, message):
    if not condition:
        raise ValueError(message)


def trusted_runtime(isolated=True):
    require(sys.implementation.name == "cpython" and sys.version_info[:3] == (3, 12, 11),
            "trusted exact CPython 3.12.11 required")
    if isolated:
        require(sys.flags.isolated and sys.flags.no_site and sys.flags.dont_write_bytecode,
                "oracle requires -I -S -B")


def pairs(items):
    result = {}
    for key, value in items:
        require(key not in result, "duplicate JSON key: " + key)
        result[key] = value
    return result


def decode_blobs(blobs):
    require(set(blobs) == set(FILES), "unexpected or missing input files")
    total, decoded = 0, []
    for name in FILES:
        raw = blobs[name]
        require(isinstance(raw, bytes) and len(raw) <= LIMIT and b"\0" not in raw,
                name + ": bounded text (2 MiB) required")
        total += len(raw)
        require(total <= TOTAL_LIMIT, "32 MiB aggregate cap")
        decoded.append(json.loads(raw.decode("utf-8"), object_pairs_hook=pairs,
                                  parse_constant=lambda value: require(False, "invalid JSON constant")))
    return decoded


def read_blobs():
    blobs = {}
    total = 0
    for name in FILES:
        path = ROOT / name
        require(not path.is_symlink(), "symlink input refused: " + name)
        with path.open("rb") as stream:
            raw = stream.read(LIMIT + 1)
        total += len(raw)
        require(len(raw) <= LIMIT, name + ": bounded text (2 MiB) required")
        require(total <= TOTAL_LIMIT, "32 MiB aggregate cap")
        blobs[name] = raw
    return blobs


def exact_keys(value, keys, context):
    require(isinstance(value, dict) and set(value) == set(keys), context + ": unknown or missing JSON keys")


def validate_inventory(contract):
    exact_keys(contract, ("profile", "scope", "standards", "field_rule_columns", "schema_notes", "attribute_policy", "rust_handlers", "witness_groups", "classes", "order_obligations"), "contract")
    require(contract["profile"] == PROFILE, "profile narrowing or mismatch")
    require(contract["scope"] == "finite G2/G3 inventory only; not G4-G6 or production cutover", "scope narrowing")
    require(contract["standards"] == ["CPython 3.12.11 ast class _fields and _attributes metadata", "Hand-reviewed Python 3.12 ASDL field-type catalog in check_python_visitor_contract.py", "src/python_extract.py#Extractor", "src/python_extract.py#expression"], "standards mismatch")
    require(contract["field_rule_columns"] == ["type", "disposition", "edge_proof"], "field rule columns")
    require(contract["schema_notes"] == SCHEMA_NOTES, "schema notes")
    require(contract["attribute_policy"] == "normalized span metadata; grammar/location parity remains G4", "attribute policy")
    require(contract["rust_handlers"] == HANDLERS, "Rust handler references")
    require(contract["witness_groups"] == {"module": {"required_ids": list(REQUIRED_IDS), "include_additional_cases": True}}, "witness group narrowing")
    expected = {row["name"]: row for row in reviewed_inventory()}
    rows = contract["classes"]
    require(isinstance(rows, list), "classes must be a list")
    seen = set()
    for row in rows:
        exact_keys(row, next(iter(expected.values())), "class")
        name = row["name"]
        require(isinstance(name, str) and name in expected, "unexpected class")
        require(name not in seen, "duplicate class: " + name)
        seen.add(name)
        require(row["ordered_fields"] == expected[name]["ordered_fields"], name + ": missing, duplicate, unexpected or reordered fields")
        require(row["field_rules"] == expected[name]["field_rules"], name + ": invalid field type/disposition/proof")
        require(row == expected[name], name + ": class policy mismatch")
    require(seen == set(expected), "missing classes: " + ", ".join(sorted(set(expected) - seen)))
    require(contract["order_obligations"] == ORDER_OBLIGATIONS, "order obligations missing or narrowed")
    return rows


# These declarations are reference-test obligations, not conclusions from ast.walk.
# The raw helper/Ruff oracle must compare complete facts/diagnostics/observations.
ORDER_OBLIGATIONS = [
    {"id": "headers", "cases": ["g2_declarations", "g2_argument_order", "g2_type_params"], "handler": "definitions", "description": "definition metadata first; args, body, decorators, returns, type_params visits; arguments posonlyargs,args,vararg,kwonlyargs,kw_defaults,kwarg,defaults", "groups": ["FunctionDef.args", "FunctionDef.body", "FunctionDef.decorator_list", "FunctionDef.returns", "FunctionDef.type_params"]},
    {"id": "async-headers", "cases": ["g2_declarations"], "handler": "definitions", "description": "async definitions preserve the same metadata and deferred header visit groups", "groups": ["AsyncFunctionDef.args", "AsyncFunctionDef.body", "AsyncFunctionDef.decorator_list", "AsyncFunctionDef.returns", "AsyncFunctionDef.type_params"]},
    {"id": "class-headers", "cases": ["g2_declarations"], "handler": "definitions", "description": "class metadata first; bases, keywords, body, decorators, type_params visits; class decorators do not run discovery", "groups": ["ClassDef.bases", "ClassDef.keywords", "ClassDef.body", "ClassDef.decorator_list", "ClassDef.type_params"]},
    {"id": "argument-groups", "cases": ["g2_argument_order"], "handler": "definitions", "description": "argument AST traversal differs from parameter fact emission: annotations including vararg precede kw_defaults; kwarg annotation precedes positional defaults", "groups": ["arguments.posonlyargs", "arguments.args", "arguments.vararg", "arguments.kwonlyargs", "arguments.kw_defaults", "arguments.kwarg", "arguments.defaults"]},
    {"id": "dict-groups", "cases": ["g2_expressions"], "handler": "expressions", "description": "all Dict.keys before all Dict.values, not source interleaving", "groups": ["Dict.keys", "Dict.values"]},
    {"id": "call-groups", "cases": ["g2_interleaved_args"], "handler": "expressions", "description": "Call.func then all args then all keywords despite interleaved source spelling", "groups": ["Call.func", "Call.args", "Call.keywords"]},
    {"id": "mapping-groups", "cases": ["g2_patterns"], "handler": "patterns", "description": "all MatchMapping.keys before all MatchMapping.patterns", "groups": ["MatchMapping.keys", "MatchMapping.patterns"]},
    {"id": "comprehensions", "cases": ["g2_comprehensions"], "handler": "expressions", "description": "element/key/value before generators; each comprehension target,iter,ifs", "groups": ["comprehension.target", "comprehension.iter", "comprehension.ifs"]},
    {"id": "metadata", "cases": ["g3_metadata_order"], "handler": "definitions", "description": "decorator discovery precedes signature/body; body imports affect deferred header call resolution; include metadata immediately precedes call; owner restored", "groups": ["FunctionDef.body", "FunctionDef.decorator_list", "FunctionDef.returns"]},
    {"id": "render-visit", "cases": ["g2_render_caps"], "handler": "expressions", "description": "render clipping (16 children,80 nodes,depth12,512 bytes) must not clip visit descendants", "groups": ["Call.args"]},
]


def metadata_gate(rows):
    modern = set()
    bases = [getattr(ast, name) for name in CATALOG if name != "support"]
    support = {row["name"] for row in rows if row["category"] == "support"}
    for name, cls in vars(ast).items():
        if isinstance(cls, type) and name not in LEGACY and (cls.__base__ in bases or name in support):
            modern.add(name)
    require(modern == {row["name"] for row in rows}, "runtime canonical class set mismatch")
    for row in rows:
        cls = getattr(ast, row["name"])
        require(list(cls._fields) == row["ordered_fields"], row["name"] + ": runtime _fields mismatch")
        require(list(cls._attributes) == row["attributes"], row["name"] + ": runtime _attributes mismatch")


def child_nodes(value):
    if isinstance(value, ast.AST):
        return [value]
    if isinstance(value, list):
        return [item for item in value if isinstance(item, ast.AST)]
    return []


def call_labels(nodes):
    return [node.func.id for root in nodes for node in ast.walk(root)
            if isinstance(node, ast.Call) and isinstance(node.func, ast.Name)]


def coverage(rows, witnesses):
    exact_keys(witnesses, ("profile", "cases"), "witnesses")
    require(witnesses["profile"] == PROFILE, "witness profile narrowing or mismatch")
    require(isinstance(witnesses["cases"], list), "witness cases must be a list")
    trees, labels = {}, {}
    for case in witnesses["cases"]:
        exact_keys(case, ("id", "source"), "witness case")
        key, source = case["id"], case["source"]
        require(isinstance(key, str) and key and isinstance(source, str), "witness id/source must be strings")
        require(key not in trees, "duplicate witness id: " + key)
        require(len(source.encode("utf-8")) <= LIMIT, "witness source bounded text")
        # This is the sole source parser. AST-only accepts some non-compilable
        # constructs deliberately; never compile, eval, exec or import a case.
        trees[key] = ast.parse(source, filename="<synthetic:" + key + ">", mode="exec", type_comments=True)
        counts = {}
        for label in call_labels([trees[key]]):
            counts[label] = counts.get(label, 0) + 1
        labels[key] = counts
    require(set(REQUIRED_IDS) <= trees.keys(), "missing witness cases: " + ", ".join(sorted(set(REQUIRED_IDS) - trees.keys())))
    by_class = {}
    for key, tree in trees.items():
        for node in ast.walk(tree):
            by_class.setdefault(type(node).__name__, []).append((key, node))
    missing_classes, missing_edges = [], []
    node_fields = observable = structural = 0
    for row in rows:
        if row["domain"] != "Module":
            continue
        name = row["name"]
        occurrences = by_class.get(name, [])
        if not occurrences:
            missing_classes.append(name)
        for field, (kind, _disposition, proof) in zip(row["ordered_fields"], row["field_rules"]):
            if kind.rstrip("?*") in SCALARS:
                continue
            node_fields += 1
            observable += proof == "unique-call"
            structural += proof == "structural"
            edge = name + "." + field
            found = False
            for key, node in occurrences:
                children = child_nodes(getattr(node, field))
                if not children:
                    continue
                tags = [tag for tag in call_labels(children) if labels[key][tag] == 1]
                if proof == "structural" or tags:
                    found = True
                    break
            if not found:
                missing_edges.append(edge + " (" + proof + ")")
    # Require distinct edge labels within one parent occurrence for each explicit
    # group-order obligation. This establishes observable inputs, NOT output order.
    missing_orders = []
    for order in ORDER_OBLIGATIONS:
        parent = order["groups"][0].split(".")[0]
        observable_parent = False
        for key, node in by_class.get(parent, []):
            if key not in order["cases"]:
                continue
            groups = []
            for edge in order["groups"]:
                owner, field = edge.split(".")
                require(owner == parent, "order groups must share a parent class")
                groups.append({tag for tag in call_labels(child_nodes(getattr(node, field)))
                               if labels[key][tag] == 1})
            if all(groups) and sum(map(len, groups)) == len(set().union(*groups)):
                observable_parent = True
                break
        if not observable_parent:
            missing_orders.append(order["id"] + ":" + ",".join(order["groups"]))
    require(not (missing_classes or missing_edges or missing_orders),
            "witness coverage pending; classes=" + ",".join(missing_classes)
            + "; edges=" + ",".join(missing_edges)
            + "; order inputs=" + ",".join(missing_orders))
    return {"classes": len(rows), "module_classes": sum(row["domain"] == "Module" for row in rows), "node_fields": node_fields,
            "observable_edges": observable, "structural_edges": structural,
            "cases": len(trees), "order_obligations": len(ORDER_OBLIGATIONS),
            "order_parity": "not certified: separate complete helper/Ruff raw oracle required"}


def validate(blobs):
    trusted_runtime()
    contract, witnesses = decode_blobs(blobs)
    rows = validate_inventory(contract)
    metadata_gate(rows)
    return coverage(rows, witnesses)


def main():
    try:
        print(json.dumps(validate(read_blobs()), sort_keys=True))
    except (ValueError, OSError, SyntaxError, RecursionError) as error:
        print("python visitor contract: " + str(error), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
