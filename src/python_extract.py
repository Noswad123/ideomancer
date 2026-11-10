"""Trusted AST-only helper. Input contains bytes, never executable target paths.

Dynamic discovery diagnostics describe a static-analysis limitation, not a failed
scan. Only unavailable/unparseable source makes the result incomplete.
Assignment evidence covers Assign/AnnAssign, not AugAssign/NamedExpr or general
control-flow ordering. Destructuring invalidates targets without inferring a map.
Discovery candidates do not prove framework identity or runtime activation. Only
bounded API paths and direct framework prefixes are retained;
payload, configuration, description, and other expression literals stay redacted.
"""

import ast
import hashlib
import json
import sys
import re


def expression(node, depth=0, budget=None):
    """Describe expression shape without copying literal bodies or source text."""
    if budget is None:
        budget = [80]
    budget[0] -= 1
    if node is None:
        return ""
    if budget[0] < 0 or depth > 12:
        return "..."

    def child(value):
        return expression(value, depth + 1, budget)

    if isinstance(node, ast.Name):
        result = node.id
    elif isinstance(node, ast.Attribute):
        result = child(node.value) + "." + node.attr
    elif isinstance(node, ast.Constant):
        result = "<literal>"
    elif isinstance(node, ast.Call):
        # Reserve the target before argument traversal can exhaust the shared
        # shape budget. Keep the existing depth and final byte bounds.
        callee = child(node.func)
        args = [child(arg) for arg in node.args[:16]]
        if len(node.args) > 16:
            args.append("...")
        args += [(kw.arg or "**") + "=" + child(kw.value) for kw in node.keywords[:16]]
        if len(node.keywords) > 16:
            args.append("...")
        result = callee + "(" + ", ".join(args) + ")"
    elif isinstance(node, ast.Subscript):
        result = child(node.value) + "[" + child(node.slice) + "]"
    elif isinstance(node, (ast.Tuple, ast.List, ast.Set)):
        result = "[" + ", ".join(child(item) for item in node.elts[:16]) + "]"
    elif isinstance(node, ast.BinOp):
        result = child(node.left) + " " + type(node.op).__name__ + " " + child(node.right)
    elif isinstance(node, ast.Starred):
        result = "*" + child(node.value)
    else:
        result = "<" + type(node).__name__ + ">"
    return result.encode("utf-8")[:512].decode("utf-8", errors="ignore")


class Extractor(ast.NodeVisitor):
    def __init__(self, path, module, facts, diagnostics):
        self.path = path
        self.module = module
        self.scope = []
        self.scope_kinds = []
        self.facts = facts
        self.diagnostics = diagnostics
        self.aliases = {}

    def fact(self, kind, node, value, symbol=None):
        symbol = symbol if symbol is not None else ".".join([self.module] + self.scope)
        line = getattr(node, "lineno", 1)
        end = getattr(node, "end_lineno", line) or line
        identity = json.dumps([self.path, kind, line, end, symbol, value], ensure_ascii=True)
        # Module IDs depend only on relative path, not code or interpreter.
        key = self.path if kind == "module" else identity
        prefix = "module_" if kind == "module" else "fact_"
        self.facts.append(dict(id=prefix + hashlib.sha256(key.encode()).hexdigest(),
                               kind=kind, path=self.path, line=line, endLine=end,
                               symbol=symbol, value=value))

    def definition(self, node, kind):
        self.scope.append(node.name)
        self.scope_kinds.append(kind)
        self.fact(kind, node, node.name)
        for decorator in node.decorator_list:
            self.fact("decorator", decorator, expression(decorator))
            if not isinstance(node, ast.ClassDef):
                self.discovery(decorator)
        if isinstance(node, ast.ClassDef):
            for base in node.bases:
                self.fact("base", base, expression(base))
        else:
            signature = dict(
                positional=[arg.arg for arg in node.args.posonlyargs + node.args.args],
                positionalOnly=len(node.args.posonlyargs),
                keywordOnly=[arg.arg for arg in node.args.kwonlyargs],
            )
            for key, arg in (("vararg", node.args.vararg), ("kwarg", node.args.kwarg)):
                if arg is not None:
                    signature[key] = arg.arg
            # Never truncate a signature into a misleading valid argument map.
            value = json.dumps(signature, sort_keys=True, separators=(",", ":"))
            if len(node.args.posonlyargs + node.args.args + node.args.kwonlyargs) > 128 or len(value.encode()) > 16384:
                value = '{"truncated":true}'
            self.fact("signature", node, value)
            args = node.args.posonlyargs + node.args.args
            defaults = [None] * (len(args) - len(node.args.defaults)) + node.args.defaults
            args += node.args.kwonlyargs
            defaults += node.args.kw_defaults
            for extra in (node.args.vararg, node.args.kwarg):
                if extra is not None:
                    args.append(extra)
                    defaults.append(None)
            for arg, default in zip(args, defaults):
                symbol = ".".join([self.module] + self.scope + [arg.arg])
                self.fact("parameter", arg, arg.arg, symbol)
                if arg.annotation is not None:
                    self.fact("parameter_annotation", arg, expression(arg.annotation), symbol)
                if default is not None:
                    self.fact("parameter_default", default, expression(default), symbol)
            if node.returns is not None:
                self.fact("return_annotation", node.returns, expression(node.returns))
        self.generic_visit(node)
        self.scope.pop()
        self.scope_kinds.pop()

    def discovery(self, decorator):
        called = isinstance(decorator, ast.Call)
        func = decorator.func if called else decorator
        attr = func.attr if isinstance(func, ast.Attribute) else None
        role = None
        if called and attr == "command":
            role = "cli_command"
        elif (called and attr in {"task", "job", "scheduled_job"}) or (
            isinstance(func, ast.Name) and func.id == "task"
        ):
            role = "job"
        if role:
            self.fact("entrypoint", decorator, json.dumps(
                dict(role=role, decorator=expression(func)), sort_keys=True, separators=(",", ":")))

        http = {"GET", "POST", "PUT", "PATCH", "DELETE", "OPTIONS", "HEAD", "TRACE"}
        if not called or attr not in {method.lower() for method in http} | {"api_route", "route"}:
            return
        # Inspect only this decorator's explicit contract arguments, never bindings
        # or router prefixes. In particular, body client.get calls are not routes.
        paths = [kw.value for kw in decorator.keywords if kw.arg == "path"]
        if decorator.args:
            paths.insert(0, decorator.args[0])
        path = paths[0] if len(paths) == 1 else None
        valid_path = (isinstance(path, ast.Constant) and isinstance(path.value, str)
                      and (path.value == "" or path.value.startswith("/")) and len(path.value) <= 2048)
        if not valid_path:
            self.diagnostics.append(dict(
                code="dynamic_api_route", path=self.path,
                message=f"line {decorator.lineno}: route path is not a bounded direct slash-prefixed string; prefix not inferred"))
        methods = [attr.upper()] if attr.upper() in http else []
        if attr in {"api_route", "route"}:
            values = [kw.value for kw in decorator.keywords if kw.arg == "methods"]
            value = values[0] if len(values) == 1 else None
            if isinstance(value, (ast.List, ast.Tuple, ast.Set)) and all(
                isinstance(item, ast.Constant) and isinstance(item.value, str)
                and item.value.upper() in http for item in value.elts
            ):
                methods = sorted({item.value.upper() for item in value.elts})
            else:
                self.diagnostics.append(dict(
                    code="dynamic_api_route", path=self.path,
                    message=f"line {decorator.lineno}: route methods are not a literal HTTP whitelist; methods unknown"))
        self.fact("api_route", decorator, json.dumps(dict(
            methods=methods, path=path.value if valid_path else None,
            receiver=expression(func.value), pathIsFragment=True,
        ), sort_keys=True, separators=(",", ":")))

    def visit_ClassDef(self, node):
        self.definition(node, "class")

    def router_constructor(self, func, constructor="APIRouter"):
        name = expression(func)
        head, dot, tail = name.partition(".")
        for size in range(len(self.scope), -1, -1):
            alias = self.aliases.get((tuple(self.scope[:size]), head))
            if alias is not None:
                return alias + (dot + tail if dot else "") == "fastapi." + constructor
        return name.rsplit(".", 1)[-1] == constructor

    def visit_FunctionDef(self, node):
        self.definition(node, "function")

    def visit_AsyncFunctionDef(self, node):
        self.definition(node, "async_function")

    def imports(self, node, kind, module, level):
        value = dict(module=module, level=level,
                     names=[dict(name=alias.name, asname=alias.asname) for alias in node.names])
        self.fact(kind, node, json.dumps(value, sort_keys=True, separators=(",", ":")))
        if level == 0:
            for alias in node.names:
                local = alias.asname or alias.name.split(".")[0]
                target = (module + "." if module else "") + alias.name
                if kind == "import" and not alias.asname:
                    target = local
                self.aliases[(tuple(self.scope), local)] = target

    def visit_Import(self, node):
        self.imports(node, "import", "", 0)

    def visit_ImportFrom(self, node):
        self.imports(node, "from_import", node.module or "", node.level)

    def assignment(self, node, targets):
        self.router_mutations(node, targets)
        if isinstance(node.value, ast.Call) and self.router_constructor(node.value.func, "FastAPI"):
            for target in targets:
                if isinstance(target, ast.Name) and len(target.id) <= 128:
                    self.fact("router_host", node, json.dumps(dict(target=target.id, constructor=expression(node.value.func)), sort_keys=True, separators=(",", ":")))
        if isinstance(node.value, ast.Call) and self.router_constructor(node.value.func):
            prefixes = [k.value for k in node.value.keywords if k.arg == "prefix"]
            prefix = prefixes[0] if len(prefixes) == 1 else None
            valid = (isinstance(prefix, ast.Constant) and isinstance(prefix.value, str)
                     and re.fullmatch(r"(?:/[a-z0-9_{}-]+){1,8}", prefix.value) is not None and len(prefix.value) <= 128)
            for target in targets:
                if isinstance(target, ast.Name) and len(target.id) <= 128:
                    self.fact("router_prefix", node, json.dumps(dict(target=target.id, constructor=expression(node.value.func),
                        prefix=prefix.value if valid else None, supported=valid and not node.value.args and all(k.arg in {"prefix", "tags"} for k in node.value.keywords)), sort_keys=True, separators=(",", ":")))
        # Syntax evidence only: do not resolve aliases or infer runtime objects.
        if node.value is not None:
            pending = [(target, False) for target in targets]
            while pending:
                target, destructured = pending.pop()
                if isinstance(target, (ast.Tuple, ast.List)):
                    pending.extend((item, True) for item in target.elts)
                    continue
                if isinstance(target, ast.Starred):
                    pending.append((target.value, True))
                    continue
                root = target
                while isinstance(root, ast.Attribute):
                    root = root.value
                if not isinstance(target, ast.Name) and not (
                    isinstance(target, ast.Attribute)
                    and isinstance(root, ast.Name) and root.id in {"self", "cls"}
                ):
                    continue
                value = dict(target=expression(target))
                kind = "binding_invalidation"
                if not destructured and isinstance(node.value, (ast.Name, ast.Attribute, ast.Call)):
                    value["source"] = expression(node.value)
                    kind = "binding"
                self.fact(kind, node, json.dumps(value, sort_keys=True, separators=(",", ":")))
        self.generic_visit(node)

    def visit_Assign(self, node):
        self.assignment(node, node.targets)

    def router_mutations(self, node, targets):
        for target in targets:
            if isinstance(target, ast.Attribute) and (target.attr in {"router", "prefix"} or ".router." in expression(target)):
                self.fact("router_mutation", node, json.dumps(dict(target=expression(target)), separators=(",", ":")))

    def visit_AugAssign(self, node):
        self.router_mutations(node, [node.target])
        self.generic_visit(node)

    def visit_Delete(self, node):
        self.router_mutations(node, node.targets)
        self.generic_visit(node)

    def visit_AnnAssign(self, node):
        if self.scope_kinds and self.scope_kinds[-1] == "class" and isinstance(node.target, ast.Name):
            symbol = ".".join([self.module] + self.scope + [node.target.id])
            self.fact("attribute_annotation", node, expression(node.annotation), symbol)
        self.assignment(node, [node.target])

    def visit_Call(self, node):
        if isinstance(node.func, ast.Attribute) and node.func.attr == "include_router":
            prefixes = [k.value for k in node.keywords if k.arg == "prefix"]
            p = prefixes[0] if len(prefixes) == 1 else None
            valid = not prefixes or (isinstance(p, ast.Constant) and isinstance(p.value, str)
                    and re.fullmatch(r"(?:/[a-z0-9_{}-]+){1,8}", p.value) is not None and len(p.value) <= 128)
            self.fact("router_include", node, json.dumps(dict(receiver=expression(node.func.value),
                router=expression(node.args[0]) if len(node.args) == 1 else None,
                prefix=p.value if valid and p is not None else "" if valid else None,
                supported=valid and len(node.args) == 1 and isinstance(node.args[0], ast.Name) and all(k.arg == "prefix" for k in node.keywords)), sort_keys=True, separators=(",", ":")))
        self.fact("call", node, expression(node))
        callee = expression(node.func)
        head, dot, tail = callee.partition(".")
        for size in range(len(self.scope), -1, -1):
            alias = self.aliases.get((tuple(self.scope[:size]), head))
            if alias is not None:
                callee = alias + (dot + tail if dot else "")
                break
        if callee.rsplit(".", 1)[-1] in {
            "__import__", "import_module", "iter_modules", "walk_packages",
            "entry_points", "load_entry_point", "find_spec", "exec", "eval",
        }:
            self.diagnostics.append(dict(code="dynamic_discovery_unresolved", path=self.path,
                                         message=f"line {node.lineno}: {callee}; runtime targets are not inferred"))
        self.generic_visit(node)


def main():
    if sys.version_info < (3, 12):
        raise SystemExit("Python >=3.12 is required; select python3.12 or python3.13 explicitly")
    request = json.load(sys.stdin)
    facts = []
    diagnostics = []
    complete = True
    for source in request["sources"]:
        path, module = source["path"], source["module"]
        visitor = Extractor(path, module, facts, diagnostics)
        # A module remains visible even if its contents cannot be parsed.
        visitor.fact("module", ast.Module(body=[], type_ignores=[]), module, module)
        start = len(facts)
        try:
            tree = ast.parse(bytes.fromhex(source["hex"]), filename=path, type_comments=True)
            visitor.visit(tree)
        except (SyntaxError, ValueError, RecursionError, MemoryError, UnicodeError) as error:
            del facts[start:]
            complete = False
            line = getattr(error, "lineno", None)
            diagnostics.append(dict(code="parse_failure", path=path,
                                    message=f"{type(error).__name__} at line {line or 0}; source not analyzed"))
    # Identical repeated facts (e.g. same-line calls) carry no extra evidence.
    facts = list({fact["id"]: fact for fact in facts}.values())
    facts.sort(key=lambda f: (f["path"], f["line"], f["endLine"], f["kind"], f["symbol"], f["value"], f["id"]))
    diagnostics.sort(key=lambda d: (d["path"], d["code"], d["message"]))
    json.dump(dict(pythonVersion=sys.version.split()[0], facts=facts,
                   diagnostics=diagnostics, complete=complete), sys.stdout, ensure_ascii=True)


if __name__ == "__main__":
    main()
