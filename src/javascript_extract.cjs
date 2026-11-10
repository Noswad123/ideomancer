'use strict';

// Trusted, stdin-only syntax extractor. Never resolve or execute target modules.
const fs = require('node:fs');
const crypto = require('node:crypto');
const Module = require('node:module');
const path = require('node:path');
const { TextDecoder } = require('node:util');
const sha = value => crypto.createHash('sha256').update(value).digest('hex');
const LIMIT = { input: 40 * 1024 * 1024, output: 32 * 1024 * 1024,
  bytes: 1024 * 1024, total: 16 * 1024 * 1024, files: 5000,
  nodes: 200000, depth: 256, facts: 100000, diagnostics: 5000 };
const structuralLimit = { bytes: 1024 * 1024, facts: 4000 };
const structuralUsage = { bytes: 0, facts: 0 };
const structuralKinds = new Set(['js_factory', 'js_factory_return']);
const relationLimit = { bytes: 4 * 1024 * 1024, facts: 12000 };
const relationUsage = { bytes: 0, facts: 0 };
const relationKinds = new Set(['js_class_relation', 'js_class_scope', 'js_class_reference',
  'js_callable_signature', 'js_constructor_field']);
const optionalFact = f => structuralKinds.has(f.kind) || relationKinds.has(f.kind)
  || (f.kind === 'js_module_export' && JSON.parse(f.value).policy === 'javascript-structural-facts-1');
const result = { protocolVersion: 'ideomancer.js-extractor.v1', nodeVersion: process.version,
  typescriptVersion: '', typescriptSha256: '', facts: [], diagnostics: [], complete: true };
const omissionCodes = new Set(['javascript_structural_facts_omitted', 'javascript_class_relations_omitted']);
const omissions = new Set();
let ts;
function diagnostic(code, file, line = 1) {
  if (omissionCodes.has(code)) omissions.add(code);
  if (result.diagnostics.length < LIMIT.diagnostics) {
    result.diagnostics.push({ code, path: file, message: `line ${line}: ${code}` });
  }
}
function serializeResult() {
  // Capability loss must survive diagnostic floods and every output fallback.
  const markers = [...omissions].sort().map(code => ({ code, path: '', message: `line 1: ${code}` }));
  result.diagnostics = result.diagnostics
    .filter(d => d.path !== '' || !omissionCodes.has(d.code))
    .slice(0, LIMIT.diagnostics - markers.length).concat(markers);
  return JSON.stringify(result);
}
function fail(code) { const error = new Error(code); error.extractorCode = code; throw error; }
function bounded(value, bytes = 512) {
  let out = String(value);
  if (Buffer.byteLength(out) > bytes) {
    out = Buffer.from(out).subarray(0, bytes).toString('utf8').replace(/\uFFFD$/u, '');
  }
  return out;
}

function extract(source, text) {
  const file = source.path;
  const ext = path.extname(file).toLowerCase();
  const scriptKind = ext === '.tsx' ? ts.ScriptKind.TSX : ext === '.jsx' ? ts.ScriptKind.JSX
    : ['.ts', '.mts', '.cts'].includes(ext) ? ts.ScriptKind.TS : ts.ScriptKind.JS;
  const sf = ts.createSourceFile(file, text, ts.ScriptTarget.Latest, true, scriptKind);
  const facts = [];
  const structuralFacts = [];
  const relationFacts = [];
  const baselineCount = result.facts.filter(f => !optionalFact(f)).length;
  let structuralBytes = 0;
  let relationBytes = 0;
  function line(node) { return sf.getLineAndCharacterOfPosition(node.getStart(sf)).line + 1; }
  function fact(kind, node, value, symbol = source.module, destination = facts) {
    if (destination === facts && baselineCount + facts.length >= LIMIT.facts) fail('resource_limit');
    if (destination === structuralFacts && (structuralUsage.facts + structuralFacts.length >= structuralLimit.facts
      // Reserve the following module summary; optional facts never consume baseline capacity.
      || result.facts.length + facts.length + structuralFacts.length + 2 > LIMIT.facts)) {
      fail('javascript_structural_facts_omitted');
    }
    value = typeof value === 'string' ? value : JSON.stringify(value);
    const start = node === sf ? 0 : node.getStart(sf);
    const loc = sf.getLineAndCharacterOfPosition(start);
    const endLine = sf.getLineAndCharacterOfPosition(Math.max(start, node.end - 1)).line + 1;
    const id = kind === 'module' ? `module_${sha(file)}` : `fact_${sha(JSON.stringify([
      file, kind, loc.line + 1, loc.character + 1, endLine, start, node.end, symbol, value]))}`;
    const item = { id, kind, path: file, line: loc.line + 1, endLine, symbol, value };
    if (destination === structuralFacts) {
      const bytes = Buffer.byteLength(JSON.stringify(item)) + 1;
      if (structuralUsage.bytes + structuralBytes + bytes > structuralLimit.bytes) fail('javascript_structural_facts_omitted');
      structuralBytes += bytes;
    }
    destination.push(item);
    return item;
  }
  const structuralFact = (kind, node, value, symbol) => fact(kind, node, value, symbol, structuralFacts);
  const moduleFact = fact('module', sf, source.module);
  if (sf.parseDiagnostics.length) {
    for (const d of sf.parseDiagnostics.slice(0, 20)) {
      diagnostic('javascript_parse_error', file,
        sf.getLineAndCharacterOfPosition(d.start || 0).line + 1);
    }
    result.complete = false;
    return facts;
  }
  function expr(node, depth = 0, budget = { left: 80 }) {
    if (!node) return '';
    if (--budget.left < 0 || depth > 12) return '...';
    const child = n => expr(n, depth + 1, budget);
    let value;
    if (ts.isIdentifier(node) || ts.isPrivateIdentifier(node)) value = node.text;
    else if (node.kind === ts.SyntaxKind.ThisKeyword) value = 'this';
    else if (node.kind === ts.SyntaxKind.SuperKeyword) value = 'super';
    else if (ts.isPropertyAccessExpression(node)) value = `${child(node.expression)}.${child(node.name)}`;
    else if (ts.isElementAccessExpression(node)) value = `${child(node.expression)}[${child(node.argumentExpression)}]`;
    else if (ts.isCallExpression(node) || ts.isNewExpression(node)) {
      const callee = child(node.expression);
      value = `${ts.isNewExpression(node) ? 'new ' : ''}${callee}(${(node.arguments || []).slice(0, 16).map(child).join(', ')})`;
    } else if (ts.isParenthesizedExpression(node)) value = `(${child(node.expression)})`;
    else if (ts.isExpressionWithTypeArguments(node)) value = child(node.expression);
    else if (ts.isTypeReferenceNode(node)) value = child(node.typeName);
    else if (ts.isQualifiedName(node)) value = `${child(node.left)}.${child(node.right)}`;
    else if (ts.isArrayLiteralExpression(node)) value = `[${node.elements.slice(0, 16).map(child).join(', ')}]`;
    else if (ts.isBinaryExpression(node)) value = `${child(node.left)} <operator> ${child(node.right)}`;
    else if (ts.isAwaitExpression(node)) value = `await ${child(node.expression)}`;
    else if (ts.isStringLiteralLike(node) || ts.isNumericLiteral(node) || ts.isBigIntLiteral(node)
      || ts.isRegularExpressionLiteral(node) || ts.isTemplateExpression(node)
      || [ts.SyntaxKind.TrueKeyword, ts.SyntaxKind.FalseKeyword, ts.SyntaxKind.NullKeyword].includes(node.kind)) value = '<literal>';
    else value = `<${ts.SyntaxKind[node.kind] || 'syntax'}>`;
    return bounded(value);
  }
  const typeKeywords = new Set([ts.SyntaxKind.AnyKeyword, ts.SyntaxKind.UnknownKeyword,
    ts.SyntaxKind.NumberKeyword, ts.SyntaxKind.BigIntKeyword, ts.SyntaxKind.ObjectKeyword,
    ts.SyntaxKind.BooleanKeyword, ts.SyntaxKind.StringKeyword, ts.SyntaxKind.SymbolKeyword,
    ts.SyntaxKind.VoidKeyword, ts.SyntaxKind.UndefinedKeyword, ts.SyntaxKind.NeverKeyword,
    ts.SyntaxKind.IntrinsicKeyword, ts.SyntaxKind.ThisKeyword]);
  function typeExpr(node, depth = 0, budget = { left: 80 }) {
    if (!node) return '';
    if (--budget.left < 0 || depth > 12) return '...';
    const child = n => typeExpr(n, depth + 1, budget);
    const list = (items, separator) => {
      const parts = [];
      for (const item of items) {
        if (budget.left <= 0) { parts.push('...'); break; }
        parts.push(child(item));
      }
      return parts.join(separator);
    };
    let value;
    if (typeKeywords.has(node.kind)) value = ts.tokenToString(node.kind);
    else if (node.kind === ts.SyntaxKind.ThisType) value = 'this';
    else if (ts.isIdentifier(node)) value = node.text;
    else if (ts.isQualifiedName(node)) value = `${child(node.left)}.${child(node.right)}`;
    else if (ts.isTypeReferenceNode(node)) {
      value = child(node.typeName);
      if (node.typeArguments?.length) value += `<${list(node.typeArguments, ', ')}>`;
    } else if (ts.isUnionTypeNode(node)) value = list(node.types, ' | ');
    else if (ts.isIntersectionTypeNode(node)) value = list(node.types, ' & ');
    else if (ts.isArrayTypeNode(node)) value = `${child(node.elementType)}[]`;
    else if (ts.isTupleTypeNode(node)) value = `[${list(node.elements, ', ')}]`;
    else if (ts.isNamedTupleMember(node)) value = `${node.dotDotDotToken ? '...' : ''}${child(node.name)}${node.questionToken ? '?' : ''}: ${child(node.type)}`;
    else if (ts.isOptionalTypeNode(node)) value = `${child(node.type)}?`;
    else if (ts.isRestTypeNode(node)) value = `...${child(node.type)}`;
    else if (ts.isTypeOperatorNode(node)) value = `${ts.tokenToString(node.operator)} ${child(node.type)}`;
    else if (ts.isIndexedAccessTypeNode(node)) value = `${child(node.objectType)}[${child(node.indexType)}]`;
    else if (ts.isParenthesizedTypeNode(node)) value = `(${child(node.type)})`;
    else if (ts.isLiteralTypeNode(node) || ts.isTemplateLiteralTypeNode(node)) value = '<literal>';
    else if (ts.isTypeLiteralNode(node)) value = '<TypeLiteral>';
    else if (ts.isImportTypeNode(node)) value = '<ImportType>';
    else value = `<${ts.SyntaxKind[node.kind] || 'type'}>`;
    return bounded(value);
  }
  const unwrap = n => {
    while (n && (ts.isParenthesizedExpression(n) || ts.isAsExpression(n)
      || ts.isTypeAssertionExpression(n) || ts.isNonNullExpression(n) || ts.isSatisfiesExpression(n))) n = n.expression;
    return n;
  };
  const callable = n => (ts.isFunctionDeclaration(n) || ts.isFunctionExpression(n)
    || ts.isArrowFunction(n) || ts.isMethodDeclaration(n) || ts.isConstructorDeclaration(n)) && !!n.body;
  const modifier = (n, k) => !!n.modifiers?.some(m => m.kind === k);
  const visibility = n => ts.isPrivateIdentifier(n.name || sf) || modifier(n, ts.SyntaxKind.PrivateKeyword)
    ? 'private' : modifier(n, ts.SyntaxKind.ProtectedKeyword) ? 'protected' : 'public';
  const directName = n => n && (ts.isIdentifier(n) || ts.isPrivateIdentifier(n)) ? n.text : null;
  const scopes = new WeakMap();
  const bindings = [];
  const nodes = [];
  const allNodes = [];
  const functions = [];
  const classes = [];
  const calls = [];
  const assignments = [];
  const omittedNodes = new WeakSet();
  const scopesList = [];
  function scope(parent, kind, node) {
    const id = `scope_${sha(JSON.stringify([file, kind, node.getStart(sf), node.end, parent?.id || null]))}`;
    const s = { id, parent, kind, node, bindings: new Map() };
    scopesList.push(s);
    return s;
  }
  const root = scope(null, 'module', sf);
  function bind(s, name, node, kind, target) {
    if (!name) return;
    const b = { scope: s, name, node, kind, target, writes: 0, guards: [] };
    if (!s.bindings.has(name)) s.bindings.set(name, []);
    s.bindings.get(name).push(b);
    bindings.push(b);
    return b;
  }
  function pattern(s, n, node, kind, target) {
    if (ts.isIdentifier(n)) return bind(s, n.text, node, kind, target);
    if (ts.isObjectBindingPattern(n) || ts.isArrayBindingPattern(n)) {
      for (const e of n.elements) if (ts.isBindingElement(e)) pattern(s, e.name, node, kind);
    }
  }
  function lookup(s, name) {
    for (; s; s = s.parent) if (s.bindings.has(name)) return s.bindings.get(name);
    return null;
  }
  function hasUseStrict(statements) {
    for (const statement of statements || []) {
      if (!ts.isExpressionStatement(statement) || !ts.isStringLiteral(statement.expression)) break;
      const directive = statement.expression.getText(sf);
      if (directive === "'use strict'" || directive === '"use strict"') return true;
    }
    return false;
  }
  const sourceStrict = ext === '.mjs' || ext === '.mts'
    || ts.isExternalModule(sf) || hasUseStrict(sf.statements);
  let potentialAnnexB = false;
  let count = 0;
  function walk(n, s, depth, omitted = false, strict = sourceStrict) {
    if (++count > LIMIT.nodes || depth > LIMIT.depth) fail('resource_limit');
    allNodes.push(n);
    const p = n.parent;
    // Do not model sloppy block-function runtime hoisting. A declaration's own
    // strict prologue affects its body, not its enclosing declaration environment.
    if (!strict && ts.isFunctionDeclaration(n) && n.body && p !== sf
      && !(p && ts.isBlock(p) && ts.isFunctionLike(p.parent) && p.parent.body === p)) {
      if (!potentialAnnexB) diagnostic('javascript_annex_b_binding_unresolved', file, line(n));
      potentialAnnexB = true;
    }
    if (ts.isClassDeclaration(n) || ts.isClassExpression(n)
      || (ts.isFunctionLike(n) && n.body && ts.isBlock(n.body) && hasUseStrict(n.body.statements))) strict = true;
    const boundary = ts.isGetAccessorDeclaration(n) || ts.isSetAccessorDeclaration(n)
      || ts.isClassStaticBlockDeclaration(n) || ts.isHeritageClause(n)
      || ts.isComputedPropertyName(n) || ts.isDecorator(n)
      || (p && ts.isParameter(p) && (p.initializer === n
        || (p.name === n && !ts.isIdentifier(n))))
      || (p && ts.isPropertyDeclaration(p) && p.initializer === n && !callable(unwrap(n)));
    if (!omitted && boundary) diagnostic('javascript_unsupported_execution_boundary', file, line(n));
    omitted = omitted || boundary;
    // Keep binding/write analysis in omitted subtrees, never their callable facts.
    if (omitted) omittedNodes.add(n);
    else nodes.push(n);
    if (ts.isFunctionDeclaration(n) && n.name) bind(s, n.name.text, n, 'function', n.body ? n : null);
    if (ts.isClassDeclaration(n) && n.name) bind(s, n.name.text, n, 'class');
    if ((ts.isEnumDeclaration(n) || ts.isModuleDeclaration(n)) && ts.isIdentifier(n.name)) bind(s, n.name.text, n, 'unsupported');
    if (ts.isVariableDeclaration(n)) {
      const list = n.parent;
      const kind = list.flags & ts.NodeFlags.Const ? 'const' : list.flags & ts.NodeFlags.Let ? 'let' : 'var';
      let dest = s;
      if (kind === 'var') while (dest.parent && !['function', 'module'].includes(dest.kind)) dest = dest.parent;
      const init = unwrap(n.initializer);
      pattern(dest, n.name, n, kind, init && callable(init) ? init : null);
    }
    if (ts.isImportClause(n) && n.name) bind(s, n.name.text, n, 'import');
    if (ts.isImportSpecifier(n) || ts.isNamespaceImport(n) || ts.isImportEqualsDeclaration(n)) bind(s, n.name.text, n, 'import');
    // Body-level functions and var share this parameter environment. Default
    // initializers are omitted rather than incorrectly borrowing body bindings.
    if (ts.isFunctionLike(n)) {
      s = scope(s, 'function', n);
      if (ts.isFunctionExpression(n) && n.name) bind(s, n.name.text, n, 'function', n.body ? n : null);
      for (const p of n.parameters) pattern(s, p.name, p, 'parameter');
      if (n.body) functions.push(n);
    } else if (ts.isClassDeclaration(n) || ts.isClassExpression(n)) {
      s = scope(s, 'class', n);
      if (n.name) bind(s, n.name.text, n, 'class');
      if (!omitted) classes.push(n);
    } else if (ts.isClassStaticBlockDeclaration(n)) {
      s = scope(s, 'function', n);
    } else if ((ts.isBlock(n) && !(p && callable(p) && p.body === n))
      || ts.isModuleBlock(n) || ts.isCaseBlock(n) || ts.isForStatement(n)
      || ts.isForOfStatement(n) || ts.isForInStatement(n) || ts.isCatchClause(n)) {
      s = scope(s, 'block', n);
      if (ts.isCatchClause(n) && n.variableDeclaration) pattern(s, n.variableDeclaration.name, n.variableDeclaration, 'catch');
    }
    scopes.set(n, s);
    if (ts.isCallExpression(n) && !omitted) calls.push(n);
    if ((ts.isBinaryExpression(n) && n.operatorToken.kind >= ts.SyntaxKind.FirstAssignment
      && n.operatorToken.kind <= ts.SyntaxKind.LastAssignment)
      || ((ts.isPrefixUnaryExpression(n) || ts.isPostfixUnaryExpression(n))
        && [ts.SyntaxKind.PlusPlusToken, ts.SyntaxKind.MinusMinusToken].includes(n.operator))
      || ts.isDeleteExpression(n) || ts.isForOfStatement(n) || ts.isForInStatement(n)) assignments.push(n);
    ts.forEachChild(n, child => { walk(child, s, depth + 1, omitted, strict); });
  }
  try { walk(sf, root, 0); } catch (e) {
    if (e.extractorCode !== 'resource_limit') throw e;
    diagnostic('resource_limit', file); result.complete = false; return [moduleFact];
  }
  const mutatedProperties = new Set();
  const globalWrites = new Set();
  const propertyWrites = [];
  function write(n, s) {
    n = unwrap(n);
    if (!n) return;
    if (ts.isIdentifier(n)) {
      if (!lookup(s, n.text)) globalWrites.add(n.text);
      for (const b of lookup(s, n.text) || []) b.writes++;
    } else if (ts.isPropertyAccessExpression(n) || ts.isElementAccessExpression(n)) {
      propertyWrites.push(n);
      let base = n.expression;
      while (ts.isPropertyAccessExpression(base) || ts.isElementAccessExpression(base)) base = base.expression;
      if (ts.isIdentifier(base)) for (const b of lookup(s, base.text) || []) mutatedProperties.add(b);
    } else if (ts.isObjectLiteralExpression(n) || ts.isArrayLiteralExpression(n)) {
      ts.forEachChild(n, c => {
        if (ts.isPropertyAssignment(c)) write(c.initializer, s);
        else if (ts.isShorthandPropertyAssignment(c)) write(c.name, s);
        else if (ts.isSpreadAssignment(c) || ts.isSpreadElement(c)) write(c.expression, s);
        else write(c, s);
      });
    } else if (ts.isBinaryExpression(n)) write(n.left, s);
  }
  for (const n of assignments) write(ts.isBinaryExpression(n) ? n.left
    : ts.isForOfStatement(n) || ts.isForInStatement(n) ? n.initializer : n.expression || n.operand, scopes.get(n));

  const declarations = new WeakMap();
  const parameterFacts = new WeakMap();
  const baseFacts = new WeakMap();
  function owner(n, includeClass = true) {
    for (let p = n.parent; p; p = p.parent) {
      if (declarations.has(p) && (includeClass || callable(p))) return declarations.get(p).fact;
      if (ts.isFunctionLike(p) || ts.isClassStaticBlockDeclaration(p)
        || ts.isHeritageClause(p) || ts.isParameter(p)
        || (ts.isPropertyDeclaration(p) && p.initializer && !callable(unwrap(p.initializer)))) return null;
    }
    return moduleFact;
  }
  const exportAssignments = nodes.filter(n => ts.isBinaryExpression(n)
    && n.operatorToken.kind === ts.SyntaxKind.EqualsToken
    && ts.isPropertyAccessExpression(n.left) && ts.isIdentifier(n.left.expression)
    && n.left.expression.text === 'module' && n.left.name.text === 'exports');
  const isExportReference = n => {
    let base = n;
    while (ts.isPropertyAccessExpression(base) || ts.isElementAccessExpression(base)) base = base.expression;
    return ts.isIdentifier(base) && ['module', 'exports'].includes(base.text);
  };
  const exportWrites = propertyWrites.filter(isExportReference);
  const dynamicScope = nodes.some(n => ts.isWithStatement(n) || (ts.isCallExpression(n)
    && ts.isIdentifier(n.expression) && n.expression.text === 'eval' && !lookup(scopes.get(n), 'eval')));
  const exportNode = exportAssignments.length === 1 && exportWrites.length === 1 ? exportAssignments[0] : null;
  const exportScope = exportNode && scopes.get(exportNode);
  const factory = exportNode && !dynamicScope && !globalWrites.has('module') && !globalWrites.has('exports')
    && !lookup(exportScope, 'module') && !lookup(exportScope, 'exports')
    && callable(unwrap(exportNode.right)) ? unwrap(exportNode.right) : null;
  function declaration(n) {
    const isClass = ts.isClassDeclaration(n) || ts.isClassExpression(n);
    const isConstructor = ts.isConstructorDeclaration(n);
    const parent = n.parent;
    const namedVariable = ts.isVariableDeclaration(parent) && directName(parent.name);
    const arrowField = ts.isPropertyDeclaration(parent) && directName(parent.name);
    const method = ts.isMethodDeclaration(n) || !!arrowField;
    let name = namedVariable || arrowField || directName(n.name);
    if (isConstructor) name = 'constructor';
    if (!name) {
      const loc = sf.getLineAndCharacterOfPosition(n.getStart(sf));
      name = `${isClass ? 'class' : n === factory ? 'factory' : 'callback'}_L${loc.line + 1}_C${loc.character + 1}`;
    }
    const own = owner(n);
    if (!own) return;
    const symbol = `${own.symbol}.${name}`;
    const vis = visibility(arrowField ? parent : n);
    const f = fact(isClass ? 'class' : modifier(n, ts.SyntaxKind.AsyncKeyword) ? 'async_function' : 'function', n, name, symbol);
    let kind = isClass ? 'class' : isConstructor ? 'constructor' : n === factory ? 'factory'
      : method ? 'method' : namedVariable || ts.isFunctionDeclaration(n) ? (ts.isArrowFunction(n) ? 'arrow' : 'function') : 'callback';
    const meta = fact('js_declaration', n, { declarationFactId: f.id, ownerFactId: own.id, kind, name,
      ...(isClass ? { classForm: ts.isClassDeclaration(n) ? 'class_declaration' : 'class_expression',
        start: n.getStart(sf), end: n.end } : {}),
      publicCandidate: vis === 'public' && !isConstructor && kind !== 'callback'
        && (!method || !!directName(n.name) || !!arrowField), visibility: vis,
      ...(method ? { static: modifier(arrowField ? parent : n, ts.SyntaxKind.StaticKeyword) } : {}) }, symbol);
    declarations.set(n, { fact: f, metadata: meta, kind });
    if (isClass) {
      for (const clause of n.heritageClauses || []) {
        if (clause.token !== ts.SyntaxKind.ExtendsKeyword) continue;
        for (const type of clause.types) baseFacts.set(type, fact('base', type, expr(type), symbol));
      }
    } else {
      for (const p of n.parameters) {
        const pname = directName(p.name) || '<binding>';
        parameterFacts.set(p, fact('parameter', p, pname, `${symbol}.${pname}`));
        if (p.type) fact('parameter_annotation', p, expr(p.type), `${symbol}.${pname}`);
      }
      if (n.type) fact('return_annotation', n.type, expr(n.type), symbol);
    }
  }
  // Preorder ensures that each declaration refers to its actual containing owner.
  const classNodes = new Set(classes);
  for (const n of nodes) if (classNodes.has(n) || callable(n)) declaration(n);
  for (const b of bindings) {
    if (omittedNodes.has(b.node)) continue;
    b.guards.push(fact('js_binding_guard', b.node, { name: b.name, bindingKind: b.kind,
      scopeId: b.scope.id, scopeKind: b.scope.kind,
      scopeStart: b.scope.node.getStart(sf), scopeEnd: b.scope.node.end,
      declarationCount: b.scope.bindings.get(b.name).length, writes: b.writes,
      ...(b.kind === 'class' || ts.isVariableDeclaration(b.node) ? { initializationEnd: b.node.end }
        : b.target && ts.isFunctionDeclaration(b.node) ? { initializationEnd: null } : {}),
      declarationFactId: b.target && declarations.get(b.target)?.fact.id || null }).id);
  }
  const memberCallables = new WeakSet();
  for (const c of classes) {
    const cf = declarations.get(c).fact;
    function member(n, memberKind, target, name = directName(n.name)) {
      if (!name) return;
      fact('js_member', n, { ownerFactId: cf.id,
        ...(target && declarations.has(target) ? { declarationFactId: declarations.get(target).fact.id } : {}), name,
        memberKind, visibility: visibility(n), static: modifier(n, ts.SyntaxKind.StaticKeyword),
        ...(n.type ? { type: typeExpr(n.type) } : {}) }, `${cf.symbol}.${name}`);
      if (target && declarations.has(target)) memberCallables.add(target);
    }
    for (const m of c.members) {
      if (ts.isConstructorDeclaration(m)) {
        member(m, 'constructor', m.body ? m : null, 'constructor');
        for (const p of m.parameters) if (p.modifiers?.some(x => [ts.SyntaxKind.PublicKeyword,
          ts.SyntaxKind.PrivateKeyword, ts.SyntaxKind.ProtectedKeyword, ts.SyntaxKind.ReadonlyKeyword].includes(x.kind))) member(p, 'field');
      } else if (ts.isMethodDeclaration(m)) member(m, 'method', m.body ? m : null);
      else if (ts.isPropertyDeclaration(m)) {
        const init = unwrap(m.initializer);
        member(m, init && callable(init) ? 'method' : 'field', init && callable(init) ? init : null);
      } else if (ts.isGetAccessorDeclaration(m) || ts.isSetAccessorDeclaration(m)) member(m, 'field');
    }
  }
  function resolve(n, name) {
    if (dynamicScope || potentialAnnexB) return null;
    const bs = lookup(scopes.get(n), name);
    if (!bs || bs.length !== 1) return null;
    const b = bs[0];
    if (b.writes || !b.target || !declarations.has(b.target) || b.guards.length !== 1) return null;
    if (JSON.parse(declarations.get(b.target).metadata.value).name !== name) return null;
    if (['const', 'let', 'var'].includes(b.kind) && n.getStart(sf) < b.node.end) return null;
    return b;
  }
  // Module proofs use the full bounded traversal, including execution boundaries
  // omitted from callable facts. These are syntax candidates, not execution proof.
  const dependencies = new Set();
  let esm = false;
  let commonjs = false;
  let importsSafe = !potentialAnnexB;
  let exportsSafe = !potentialAnnexB;
  let requireSafe = !globalWrites.has('require');
  let wrappersSafe = !globalWrites.has('module') && !globalWrites.has('exports');
  const importCandidates = [];
  const exportCandidates = [];
  const objectExports = [];
  const moduleImports = new Map();
  function dependency(n) {
    const literal = specifier(n);
    if (!literal) { importsSafe = exportsSafe = false; return null; }
    if (literal.startsWith('./') || literal.startsWith('../')) dependencies.add(literal);
    return literal;
  }
  function requireCallee(n) {
    n = unwrap(n);
    // Receiver methods are not established loader bindings, even on global names.
    return ts.isIdentifier(n) && n.text === 'require'
      && !lookup(scopes.get(n), 'require') && !globalWrites.has('require');
  }
  function rootBinding(name, node) {
    const bs = root.bindings.get(name);
    if (bs?.length !== 1) return null;
    const b = bs[0];
    return !b.writes && b.guards.length === 1 && (!node || b.node === node) ? b : null;
  }
  // Class targets must never enter the callable binding/guard proof graph.
  function classTarget(b) {
    if (!b) return null;
    const n = ts.isVariableDeclaration(b.node) ? unwrap(b.node.initializer) : b.node;
    return n && (ts.isClassDeclaration(n) || ts.isClassExpression(n)) && declarations.has(n) ? n : null;
  }
  function exportCandidate(n, exported, local, mode) {
    if (mode === 'esm' && exported === 'default') return false;
    const b = rootBinding(local);
    const c = b?.kind === 'class' && ts.isClassDeclaration(b.node) && classTarget(b);
    const d = c ? declarations.get(c) : b?.target && declarations.get(b.target);
    if (!d || !['function', 'arrow', 'class'].includes(d.kind)
      || JSON.parse(d.metadata.value).ownerFactId !== moduleFact.id) return false;
    const referenceStart = n.getStart(sf);
    if (mode === 'commonjs' && (c || ts.isVariableDeclaration(b.node)) && b.node.end > referenceStart) return false;
    exportCandidates.push({ node: n, mode, exported, local, targetFactId: d.fact.id,
      targetKind: c ? 'class' : 'function',
      targetMetadataFactId: d.metadata.id, bindingGuardFactId: b.guards[0],
      ...(mode === 'commonjs' ? { referenceStart, initializedBeforeExport: true } : {}) });
    return true;
  }
  for (const n of allNodes) {
    if (ts.isCallExpression(n) && requireCallee(n.expression)) {
      dependency(n.arguments[0]);
    }
    if (ts.isWithStatement(n) || (ts.isIdentifier(n) && n.text === 'eval'
      && !lookup(scopes.get(n), 'eval'))) importsSafe = exportsSafe = false;
    if (ts.isIdentifier(n) && n.text === 'arguments' && !lookup(scopes.get(n), 'arguments')) {
      importsSafe = exportsSafe = false;
    }
    if (ts.isIdentifier(n) && ['globalThis', 'global', 'window', 'self'].includes(n.text)
      && !((ts.isPropertyAccessExpression(n.parent) || ts.isElementAccessExpression(n.parent))
        && n.parent.expression === n)) importsSafe = exportsSafe = false;
    // Wrapper references are intentionally whitelisted narrowly. Passing them to
    // a function, aliasing them, or mutating through a property is not provable.
    if (ts.isIdentifier(n) && ['require', 'module', 'exports'].includes(n.text)) {
      const p = n.parent;
      const propertyName = (ts.isPropertyAccessExpression(p) && p.name === n)
        || ((ts.isPropertyAssignment(p) || ts.isBindingElement(p)) && p.name === n && p.initializer !== n)
        || (ts.isBindingElement(p) && p.propertyName === n)
        || (ts.isImportSpecifier(p) && p.propertyName === n)
        || (ts.isExportSpecifier(p) && p.name === n && p.propertyName);
      if (!propertyName) {
        commonjs = true;
        if (n.text === 'require') {
          if (lookup(scopes.get(n), 'require') || !(ts.isCallExpression(p) && p.expression === n)) requireSafe = false;
        } else if (lookup(scopes.get(n), n.text) || !(n.text === 'module'
          && ts.isPropertyAccessExpression(p) && p.expression === n && p.name.text === 'exports'
          && ts.isBinaryExpression(p.parent) && p.parent.left === p
          && p.parent.operatorToken.kind === ts.SyntaxKind.EqualsToken
          && ts.isExpressionStatement(p.parent.parent) && p.parent.parent.parent === sf
          && (ts.isObjectLiteralExpression(p.parent.right)
            || callable(unwrap(p.parent.right))))) wrappersSafe = false;
      }
    }
    if ((ts.isPropertyAccessExpression(n) || ts.isElementAccessExpression(n))
      && ts.isIdentifier(n.expression) && ['globalThis', 'global', 'window', 'self'].includes(n.expression.text)) {
      const key = ts.isPropertyAccessExpression(n) ? n.name.text
        : ts.isStringLiteralLike(n.argumentExpression || sf) ? n.argumentExpression.text : null;
      if (key === 'require' || key === null) requireSafe = false;
      if (['module', 'exports', 'eval'].includes(key) || key === null) importsSafe = exportsSafe = false;
    }
    if (ts.isImportDeclaration(n)) {
      esm = true;
      const moduleSpecifier = dependency(n.moduleSpecifier);
      const clause = n.importClause;
      if (n.parent !== sf || !ts.isStringLiteral(n.moduleSpecifier) || n.attributes || n.assertClause
        || n.phaseModifier) importsSafe = false;
      if (clause && !clause.isTypeOnly && clause.namedBindings && ts.isNamedImports(clause.namedBindings)) {
        for (const item of clause.namedBindings.elements) {
          if (item.isTypeOnly) continue;
          const imported = directName(item.propertyName || item.name);
          const b = rootBinding(item.name.text, item);
          if (moduleSpecifier && imported && imported !== 'default' && b) importCandidates.push({ node: item, b, mode: 'esm',
            moduleSpecifier, imported, local: item.name.text });
        }
      }
    } else if (ts.isImportTypeNode(n)) {
      if (ts.isLiteralTypeNode(n.argument)) dependency(n.argument.literal);
    } else if (ts.isImportEqualsDeclaration(n)) {
      if (ts.isExternalModuleReference(n.moduleReference)) dependency(n.moduleReference.expression);
      commonjs = true;
      if (!n.isTypeOnly) importsSafe = false;
    } else if (ts.isCallExpression(n) && (n.expression.kind === ts.SyntaxKind.ImportKeyword
      || (ts.isIdentifier(n.expression) && n.expression.text === 'require'))) {
      if (n.expression.kind !== ts.SyntaxKind.ImportKeyword && !requireCallee(n.expression)) {
        importsSafe = exportsSafe = false;
        continue;
      }
      const moduleSpecifier = dependency(n.arguments[0]);
      if (n.expression.kind === ts.SyntaxKind.ImportKeyword) {
        esm = true; importsSafe = exportsSafe = false;
        continue;
      }
      commonjs = true;
      if (n.arguments.length !== 1 || !ts.isStringLiteral(n.arguments[0])) importsSafe = exportsSafe = false;
      const v = n.parent;
      const list = ts.isVariableDeclaration(v) && v.parent;
      const statement = list && list.parent;
      const valid = moduleSpecifier && list && ts.isVariableDeclarationList(list) && (list.flags & ts.NodeFlags.Const)
        && ts.isVariableStatement(statement) && statement.parent === sf && v.initializer === n
        && ts.isObjectBindingPattern(v.name) && n.arguments.length === 1
        && ts.isStringLiteral(n.arguments[0]) && !n.questionDotToken
        && !lookup(scopes.get(n), 'require')
        && v.name.elements.every(e => !e.dotDotDotToken && !e.initializer && ts.isIdentifier(e.name)
          && (!e.propertyName || ts.isIdentifier(e.propertyName)));
      if (!valid) { importsSafe = false; continue; }
      for (const e of v.name.elements) {
        const b = rootBinding(e.name.text, v);
        if (b) importCandidates.push({ node: e, b, mode: 'commonjs', moduleSpecifier,
          imported: (e.propertyName || e.name).text, local: e.name.text });
      }
    }
    if (ts.isExportDeclaration(n)) {
      esm = true;
      if (n.moduleSpecifier) dependency(n.moduleSpecifier);
      if (n.isTypeOnly) continue;
      if (n.parent !== sf || n.moduleSpecifier || !n.exportClause || !ts.isNamedExports(n.exportClause)) {
        exportsSafe = false; continue;
      }
      for (const e of n.exportClause.elements) {
        if (e.isTypeOnly) continue;
        const local = directName(e.propertyName || e.name);
        const exported = directName(e.name);
        if (!local || !exported || !exportCandidate(e, exported, local, 'esm')) exportsSafe = false;
      }
    } else if (ts.isExportAssignment(n)) {
      esm = true; exportsSafe = false;
    } else if (modifier(n, ts.SyntaxKind.ExportKeyword)) {
      esm = true;
      if (modifier(n, ts.SyntaxKind.DefaultKeyword)) { exportsSafe = false; continue; }
      if (ts.isInterfaceDeclaration(n) || ts.isTypeAliasDeclaration(n)) continue;
      if (n.parent !== sf) { exportsSafe = false; continue; }
      if ((ts.isFunctionDeclaration(n) || ts.isClassDeclaration(n)) && n.name) {
        if (!exportCandidate(n, n.name.text, n.name.text, 'esm')) exportsSafe = false;
      } else if (ts.isVariableStatement(n)) {
        for (const v of n.declarationList.declarations) {
          if (!ts.isIdentifier(v.name) || !exportCandidate(v, v.name.text, v.name.text, 'esm')) exportsSafe = false;
        }
      } else exportsSafe = false;
    }
    if (ts.isBinaryExpression(n) && n.operatorToken.kind === ts.SyntaxKind.EqualsToken
      && ts.isPropertyAccessExpression(n.left) && ts.isIdentifier(n.left.expression)
      && n.left.expression.text === 'module' && n.left.name.text === 'exports') {
      commonjs = true;
      objectExports.push(n);
    }
  }
  // A factory can retain import proofs, but only an object can prove named exports.
  if (objectExports.length > 1) wrappersSafe = false;
  if (objectExports.length) {
    const n = objectExports[0];
    if (objectExports.length !== 1 || !wrappersSafe || !ts.isObjectLiteralExpression(n.right)
      || !ts.isExpressionStatement(n.parent) || n.parent.parent !== sf) exportsSafe = false;
    else {
      const names = new Set();
      for (const p of n.right.properties) {
        const exported = directName(p.name);
        const local = ts.isShorthandPropertyAssignment(p) && !p.objectAssignmentInitializer ? p.name.text
          : ts.isPropertyAssignment(p) && ts.isIdentifier(p.initializer) ? p.initializer.text : null;
        if (!exported || exported === '__proto__' || !local || names.has(exported)
          || !exportCandidate(p, exported, local, 'commonjs')) exportsSafe = false;
        if (exported) names.add(exported);
      }
    }
  }
  importsSafe = importsSafe && requireSafe && wrappersSafe;
  exportsSafe = exportsSafe && requireSafe && wrappersSafe;
  const moduleMode = esm && !commonjs ? 'esm' : commonjs && !esm ? 'commonjs' : 'unknown';
  if (esm && commonjs) importsSafe = exportsSafe = false;
  if (importsSafe) for (const { node, b, ...info } of importCandidates) {
    const f = fact('js_module_import', node, { ownerFactId: moduleFact.id, ...info,
      ...(info.mode === 'commonjs' ? { initializationEnd: b.node.end } : {}),
      bindingGuardFactId: b.guards[0] });
    moduleImports.set(b, f);
  }
  const moduleExportFacts = new Map();
  if (exportsSafe) {
    const names = new Set();
    for (const c of exportCandidates) {
      if (names.has(c.exported)) { exportsSafe = false; break; }
      names.add(c.exported);
    }
    if (exportsSafe) for (const { node, ...info } of exportCandidates) {
      moduleExportFacts.set(node, fact('js_module_export', node, { ownerFactId: moduleFact.id, ...info }));
    }
  }
  for (const n of calls) {
    const own = owner(n, false);
    if (!own) continue;
    const f = fact('call', n, expr(n), own.symbol);
    const callee = expr(n.expression); // Separate budget: argument payload never hides identity.
    const b = ts.isIdentifier(n.expression) ? resolve(n, n.expression.text) : null;
    const d = b && declarations.get(b.target);
    const importedBindings = !d && importsSafe && ts.isIdentifier(n.expression)
      ? lookup(scopes.get(n), n.expression.text) : null;
    const importedBinding = importedBindings?.length === 1 ? importedBindings[0] : null;
    const imported = importedBinding && !importedBinding.writes
      && (importedBinding.kind === 'import' || n.getStart(sf) >= importedBinding.node.end)
      ? moduleImports.get(importedBinding) : null;
    const lookupScopeIds = [];
    for (let s = scopes.get(n); s; s = s.parent) lookupScopeIds.push(s.id);
    const info = { callFactId: f.id, ownerFactId: own.id, callee, lookupScopeIds,
      ...(d ? { targetFactId: d.fact.id } : {}),
      ...(imported ? { importFactId: imported.id } : {}),
      supportingFactIds: d ? [d.fact.id, d.metadata.id, ...b.guards]
        : imported ? [imported.id, importedBinding.guards[0]] : [],
      reason: d ? 'unique stable same-file lexical function binding'
        : imported ? 'unique stable module import binding candidate'
        : potentialAnnexB ? 'UNKNOWN: possible sloppy block-function runtime binding'
          : 'UNKNOWN: no proven local lexical function binding' };
    fact('js_call', n, info, own.symbol);
    if (d) fact('js_local_call_candidate', n, info, own.symbol);
  }
  function specifier(n) {
    return n && ts.isStringLiteral(n) && n.text.length <= 512
      && /^[a-zA-Z0-9_@./$-]+$/.test(n.text) && !n.text.startsWith('//')
      && !n.text.startsWith('/') ? n.text : null;
  }
  function importFact(n, literal, mode, imported, dynamic) {
    const own = owner(n, false);
    if (!own) return;
    const spec = specifier(literal);
    if (!spec) { diagnostic('javascript_import_unresolved', file, line(n)); return; }
    fact('js_import', n, { ownerFactId: own.id, moduleSpecifier: spec,
      mode, bindings: imported, dynamic }, own.symbol);
  }
  for (const n of nodes) {
    if (ts.isImportDeclaration(n)) {
      const imported = [];
      const clause = n.importClause;
      if (clause?.name) imported.push({ imported: 'default', local: clause.name.text });
      const named = clause?.namedBindings;
      if (named && ts.isNamespaceImport(named)) imported.push({ imported: '*', local: named.name.text });
      if (named && ts.isNamedImports(named)) for (const item of named.elements) imported.push({ imported: directName(item.propertyName || item.name) || '<literal>', local: item.name.text });
      importFact(n, n.moduleSpecifier, 'esm', imported, false);
    } else if (ts.isImportEqualsDeclaration(n) && ts.isExternalModuleReference(n.moduleReference)) {
      importFact(n, n.moduleReference.expression, 'commonjs', [{ imported: '*', local: n.name.text }], false);
    } else if (ts.isCallExpression(n) && n.expression.kind === ts.SyntaxKind.ImportKeyword) {
      importFact(n, n.arguments.length === 1 ? n.arguments[0] : null, 'esm', [], true);
    } else if (ts.isCallExpression(n) && ts.isIdentifier(n.expression) && n.expression.text === 'require'
      && requireCallee(n.expression)) {
      const imported = [];
      if (ts.isVariableDeclaration(n.parent)) {
        const name = n.parent.name;
        if (ts.isIdentifier(name)) imported.push({ imported: '*', local: name.text });
        else if (ts.isObjectBindingPattern(name)) for (const e of name.elements) {
          if (ts.isIdentifier(e.name) && (!e.propertyName || ts.isIdentifier(e.propertyName))) imported.push({ imported: directName(e.propertyName || e.name), local: e.name.text });
        }
      }
      importFact(n, n.arguments.length === 1 ? n.arguments[0] : null, 'commonjs', imported, false);
    }
  }
  const methods = /^(get|post|put|patch|delete|options|head|trace|connect)$/;
  const first = factory?.parameters[0];
  const routerName = first && directName(first.name);
  const routerBindings = routerName && lookup(scopes.get(factory), routerName);
  const routerBinding = routerBindings?.length === 1 ? routerBindings[0] : null;
  // An escaped receiver may have its registration methods replaced through an alias.
  const routerEscapes = routerBinding && nodes.some(n => {
    if (!ts.isIdentifier(n) || n.text !== routerName || n === first.name) return false;
    const bs = lookup(scopes.get(n), routerName);
    if (bs?.length !== 1 || bs[0] !== routerBinding) return false;
    return !(ts.isPropertyAccessExpression(n.parent) && n.parent.expression === n);
  });
  for (const n of calls) {
    const callee = n.expression;
    if (!ts.isPropertyAccessExpression(callee) || !ts.isIdentifier(callee.expression)
      || !methods.test(callee.name.text)) continue;
    // An arbitrary SDK/client .get is not evidence of an HTTP registration.
    if (callee.expression.text !== (routerName || 'router')) continue;
    const own = owner(n, false);
    if (!own) continue;
    const bound = lookup(scopes.get(n), callee.expression.text);
    const validRouter = factory && !potentialAnnexB && routerName === 'router' && routerBinding
      && !first.dotDotDotToken && !first.initializer
      && !routerBinding.writes && !mutatedProperties.has(routerBinding) && !routerEscapes
      && bound?.length === 1 && bound[0] === routerBinding
      && own.id === declarations.get(factory).fact.id;
    let handler = n.arguments.length === 2 ? unwrap(n.arguments[1]) : null;
    if (handler && ts.isIdentifier(handler)) handler = resolve(n, handler.text)?.target;
    const hd = handler && callable(handler) && declarations.get(handler);
    const literal = n.arguments[0];
    const validPath = literal && ts.isStringLiteral(literal) && Buffer.byteLength(literal.text) <= 2048
      && (literal.text === '' || literal.text.startsWith('/'));
    if (!validRouter || !hd || !validPath || n.questionDotToken || callee.questionDotToken) {
      diagnostic('javascript_api_route_unresolved', file, line(n)); continue;
    }
    fact('api_route', n, { methods: [callee.name.text.toUpperCase()],
      path: literal.text, pathIsFragment: true, receiver: routerName, handlerFactId: hd.fact.id,
      association: 'injected-router factory registration candidate; framework/mount/activation UNKNOWN' }, own.symbol);
  }
  function structuralFactories() {
    const policy = 'javascript-structural-facts-1';
    const unsafe = potentialAnnexB || allNodes.some(n => ts.isWithStatement(n)
      || (ts.isCallExpression(n) && ts.isIdentifier(n.expression) && n.expression.text === 'eval'
        && !lookup(scopes.get(n), 'eval')));
    const candidates = [];
    const returns = new Map();
    const classBindings = new Map();
    for (const b of bindings) {
      const cls = classTarget(b);
      if (!cls) continue;
      if (!classBindings.has(cls)) classBindings.set(cls, []);
      classBindings.get(cls).push(b);
    }
    for (const n of allNodes) {
      if (!ts.isReturnStatement(n)) continue;
      let s = scopes.get(n);
      while (s && s.kind !== 'function') s = s.parent;
      if (!s) continue;
      if (!returns.has(s.node)) returns.set(s.node, []);
      returns.get(s.node).push(n);
    }
    const wrapperReferences = new Set(exportAssignments.map(n => n.left.expression));
    const structuralWrappersSafe = allNodes.every(node => {
      if (!ts.isIdentifier(node) || !['module', 'exports'].includes(node.text)) return true;
      const p = node.parent;
      if (ts.isPropertyAccessExpression(p) && p.name === node) return true;
      return wrapperReferences.has(node);
    });
    for (const n of exportAssignments) {
      const value = unwrap(n.right);
      const bs = value && ts.isIdentifier(value) ? root.bindings.get(value.text) : null;
      const b = bs?.length === 1 ? bs[0] : null;
      const fn = b ? b.target : value;
      if (!fn || !callable(fn) || !declarations.has(fn)
        || JSON.parse(declarations.get(fn).metadata.value).ownerFactId !== moduleFact.id) continue;
      const direct = ts.isExpressionStatement(n.parent) && n.parent.parent === sf;
      // CommonJS defaults stay outside the existing named-module proof graph.
      const d = declarations.get(fn);
      const initialized = !b || (!b.writes && b.guards.length === 1
        && (!ts.isVariableDeclaration(b.node) || b.node.end <= n.getStart(sf)));
      const ef = structuralFact('js_module_export', n, { policy, ownerFactId: moduleFact.id, mode: 'commonjs',
        exported: 'default', targetFactId: d.fact.id, targetKind: 'function',
        targetMetadataFactId: d.metadata.id, bindingGuardFactId: b?.guards[0] || null,
        referenceStart: n.getStart(sf), initializedBeforeExport: direct && initialized });
      const wrappersStable = wrappersSafe || (!!b && structuralWrappersSafe);
      candidates.push({ fn, node: n, exportKind: 'commonjs_default', exportedName: 'default',
        evidence: [ef.id, ...(b?.guards || [])], stable: direct && initialized
          && exportAssignments.length === 1 && exportWrites.length === 1
          && !esm && wrappersStable && !globalWrites.has('module') && !globalWrites.has('exports')
          && !lookup(scopes.get(n), 'module') && !lookup(scopes.get(n), 'exports') });
    }
    // Include uncertain named function exports too; absence is not a unique result.
    for (const n of allNodes) {
      let fn = null;
      let exportedName = null;
      if (ts.isFunctionDeclaration(n) && n.parent === sf && n.name
        && modifier(n, ts.SyntaxKind.ExportKeyword) && !modifier(n, ts.SyntaxKind.DefaultKeyword)) {
        fn = n; exportedName = n.name.text;
      } else if (ts.isVariableDeclaration(n) && ts.isIdentifier(n.name)
        && ts.isVariableStatement(n.parent.parent) && n.parent.parent.parent === sf
        && modifier(n.parent.parent, ts.SyntaxKind.ExportKeyword)) {
        const init = unwrap(n.initializer);
        if (init && callable(init)) { fn = init; exportedName = n.name.text; }
      } else if (ts.isExportSpecifier(n) && n.parent.parent.parent === sf
        && !n.isTypeOnly && !n.parent.parent.isTypeOnly && !n.parent.parent.moduleSpecifier) {
        const local = directName(n.propertyName || n.name);
        const bs = local && root.bindings.get(local);
        if (bs?.length === 1 && bs[0].target) { fn = bs[0].target; exportedName = directName(n.name); }
      }
      if (!fn || !exportedName || exportedName === 'default' || !declarations.has(fn)) continue;
      const ef = moduleExportFacts.get(n);
      candidates.push({ fn, node: n, exportKind: 'esm_named', exportedName,
        evidence: ef ? [ef.id] : [], stable: !!ef && exportsSafe && moduleMode === 'esm' });
    }
    const exportCounts = new Map();
    for (const c of candidates) exportCounts.set(c.exportedName, (exportCounts.get(c.exportedName) || 0) + 1);
    for (const c of candidates) {
      const { fn } = c;
      const d = declarations.get(fn);
      const support = new Set([moduleFact.id, d.fact.id, d.metadata.id, ...c.evidence]);
      const parameters = fn.parameters.map((p, position) => {
        const bs = ts.isIdentifier(p.name) && lookup(scopes.get(fn), p.name.text);
        const b = bs?.length === 1 && bs[0].node === p ? bs[0] : null;
        const pf = parameterFacts.get(p);
        if (pf) support.add(pf.id);
        for (const id of b?.guards || []) support.add(id);
        return { parameterFactId: pf.id, bindingGuardFactId: b?.guards[0] || null,
          name: directName(p.name) || '<binding>', position, optional: !!p.questionToken,
          rest: !!p.dotDotDotToken, hasDefault: !!p.initializer };
      });
      const block = ts.isBlock(fn.body);
      const sites = block ? returns.get(fn) || [] : [fn.body];
      const final = block ? fn.body.statements[fn.body.statements.length - 1] : fn.body;
      const returnFacts = [];
      let resultKind = 'unsupported';
      let returnedClassFactId = null;
      for (const site of sites) {
        const value = unwrap(block ? site.expression : site);
        const instance = value && ts.isNewExpression(value);
        const reference = unwrap(instance ? value.expression : value);
        let cls = reference && ts.isClassExpression(reference) ? reference : null;
        let b = null;
        if (reference && ts.isIdentifier(reference)) {
          const bs = lookup(scopes.get(reference), reference.text);
          b = bs?.length === 1 ? bs[0] : null;
          const target = classTarget(b);
          const directDeclaration = b && (b.node.parent === fn.body
            || (ts.isVariableDeclaration(b.node) && ts.isVariableStatement(b.node.parent.parent)
              && b.node.parent.parent.parent === fn.body));
          if (target && directDeclaration && b.scope.node === fn && !b.writes
            && b.guards.length === 1 && b.node.end <= site.getStart(sf)) cls = target;
        }
        // The class's own lexical name can also be reassigned inside its body.
        const guards = classBindings.get(cls) || [];
        const cd = cls && declarations.get(cls);
        const supported = !!cd && !unsafe
          && guards.every(binding => !binding.writes && binding.guards.length === 1
            && binding.scope.bindings.get(binding.name).length === 1);
        const ids = new Set([moduleFact.id, d.fact.id, d.metadata.id]);
        if (supported) {
          ids.add(cd.fact.id); ids.add(cd.metadata.id);
          for (const binding of guards) for (const id of binding.guards) ids.add(id);
        }
        const valueKind = supported ? instance ? 'instance' : 'class' : 'unsupported';
        const valueForm = reference && ts.isClassExpression(reference)
          ? instance ? 'new_class_expression' : 'class_expression'
          : reference && ts.isIdentifier(reference) ? instance ? 'new_identifier' : 'identifier' : 'unsupported';
        const rf = structuralFact('js_factory_return', site, { policy, factoryFactId: d.fact.id,
          valueKind, valueForm, valueStart: value ? value.getStart(sf) : null, valueEnd: value ? value.end : null,
          classFactId: supported ? cd.fact.id : null,
          bindingGuardFactId: supported && b ? b.guards[0] : null,
          supportingFactIds: [...ids] }, d.fact.symbol);
        returnFacts.push(rf.id);
        for (const id of ids) support.add(id);
        support.add(rf.id);
        if (sites.length === 1 && site === final) {
          resultKind = valueKind;
          returnedClassFactId = supported ? cd.fact.id : null;
        }
      }
      const ambiguous = sites.length > 1 || (sites.length === 1 && sites[0] !== final)
        || exportCounts.get(c.exportedName) > 1;
      const stable = c.stable && !unsafe
        && !fn.asteriskToken && !modifier(fn, ts.SyntaxKind.AsyncKeyword);
      if (ambiguous || !stable) {
        resultKind = ambiguous ? 'ambiguous' : 'unsupported';
        returnedClassFactId = null;
      }
      const safe = stable && !ambiguous && ['class', 'instance'].includes(resultKind);
      structuralFact('js_factory', fn, { policy, factoryFactId: d.fact.id,
        moduleFactId: moduleFact.id, modulePath: source.path, exportKind: c.exportKind,
        exportedName: c.exportedName, parameters, resultKind, returnedClassFactId,
        returnFactIds: returnFacts, supportingFactIds: [...support], safe,
        reason: safe ? null : ambiguous ? 'multiple or conditional exports/returns'
          : !stable ? 'export or callable structure is not stable'
            : 'no supported final class or instance return' }, d.fact.symbol);
    }
  }
  try {
    structuralFactories();
  } catch (error) {
    if (error.extractorCode !== 'javascript_structural_facts_omitted') throw error;
    structuralFacts.length = 0;
    structuralBytes = 0;
    diagnostic('javascript_structural_facts_omitted', file);
  }
  function classRelations() {
    const identifier = n => n && ts.isIdentifier(n) && Buffer.byteLength(n.text) <= 512;
    // Unlike callable extraction, safety checks must include omitted boundaries.
    const unsafe = potentialAnnexB || allNodes.some(n => ts.isWithStatement(n)
      || (ts.isCallExpression(n) && ts.isIdentifier(n.expression) && n.expression.text === 'eval'
        && !lookup(scopes.get(n), 'eval')));
    const unique = n => {
      if (!identifier(n)) return null;
      const bs = lookup(scopes.get(n), n.text);
      return bs?.length === 1 ? bs[0] : null;
    };
    const stable = b => !!b && !unsafe && !b.writes && b.guards.length === 1;
    const importedTarget = (b, reference) => stable(b)
      && (b.kind === 'import' || reference.getStart(sf) >= b.node.end) ? moduleImports.get(b) : null;
    const classGuards = new Map();
    for (const b of bindings) {
      const c = classTarget(b);
      if (!c) continue;
      if (!classGuards.has(c)) classGuards.set(c, []);
      classGuards.get(c).push(...b.guards);
    }
    function support(c, own, b, target, base) {
      const ids = [moduleFact.id, declarations.get(c).fact.id, ...(classGuards.get(c) || [])];
      // Include declaration/owner closure without changing any existing guard.
      for (let n = own; n; n = n.parent) {
        const d = declarations.get(n);
        if (d) ids.push(d.fact.id, d.metadata.id);
      }
      if (b) {
        ids.push(...b.guards);
        const d = declarations.get(b.node.parent);
        if (d) ids.push(d.fact.id, d.metadata.id);
      }
      if (target) {
        const d = declarations.get(target);
        ids.push(d.fact.id, d.metadata.id, ...(classGuards.get(target) || []));
      }
      if (base) ids.push(base.id);
      return [...new Set(ids)];
    }
    function parameterKind(b, c, own) {
      if (!stable(b) || b.kind !== 'parameter' || !ts.isParameter(b.node)
        || !identifier(b.node.name) || b.node.dotDotDotToken) return null;
      const fn = b.node.parent;
      if (fn === own && ts.isConstructorDeclaration(own)) return 'constructor_parameter';
      const d = declarations.get(fn);
      if (!d || ['method', 'constructor'].includes(d.kind)) return null;
      for (let n = c.parent; n; n = n.parent) if (n === fn) return 'factory_parameter';
      return null;
    }
    const scopeFacts = new Map();
    const referenceFacts = new WeakMap();
    function relationFact(kind, node, value, symbol) {
      if (relationUsage.facts + relationFacts.length >= relationLimit.facts
        || result.facts.length + facts.length + structuralFacts.length + relationFacts.length + 2 > LIMIT.facts) {
        fail('javascript_class_relations_omitted');
      }
      const f = fact(kind, node, value, symbol, relationFacts);
      const bytes = Buffer.byteLength(JSON.stringify(f)) + 1;
      if (relationUsage.bytes + relationBytes + bytes > relationLimit.bytes) fail('javascript_class_relations_omitted');
      relationBytes += bytes;
      return f;
    }
    function emit(c, own, node, relationship, expression, b, kind, target = null,
      memberName = null, declaredType = null, base = null, lookupNode = node) {
      const policy = 'javascript-class-relations-2';
      const actual = unwrap(lookupNode);
      const imported = kind === 'import' ? importedTarget(b, actual) : null;
      const lookupScopeIds = [];
      const chainFactIds = [];
      for (let s = scopes.get(actual); s; s = s.parent) {
        lookupScopeIds.push(s.id);
        if (!scopeFacts.has(s)) scopeFacts.set(s, relationFact('js_class_scope', s.node, {
          policy, scopeId: s.id, parentScopeId: s.parent?.id || null, scopeKind: s.kind,
          start: s.node.getStart(sf), end: s.node.end }, source.module));
        chainFactIds.push(scopeFacts.get(s).id);
      }
      const classFactId = declarations.get(c).fact.id;
      const ownerFactId = declarations.get(own).fact.id;
      if (!referenceFacts.has(actual)) referenceFacts.set(actual, relationFact('js_class_reference', actual, {
        policy, classFactId, ownerFactId, scopeId: scopes.get(actual).id,
        start: actual.getStart(sf), end: actual.end }, declarations.get(c).fact.symbol));
      const referenceFactId = referenceFacts.get(actual).id;
      relationFact('js_class_relation', node, { policy, classFactId, ownerFactId, referenceFactId,
        relationship, expression, lookupScopeIds, bindingKind: kind, bindingGuardFactId: b?.guards[0] || null,
        targetFactId: target ? declarations.get(target).fact.id : null,
        importFactId: imported?.id || null, memberName, declaredType,
        supportingFactIds: [...new Set([...support(c, own, b, target, base),
          ...(imported ? [imported.id] : []), referenceFactId, ...chainFactIds])],
        stableBinding: stable(b) }, declarations.get(own).fact.symbol);
    }
    const classFactIds = new Set(classes.map(c => declarations.get(c)?.fact.id).filter(Boolean));
    for (const own of functions) {
      const d = declarations.get(own);
      if (!d || !memberCallables.has(own) || !['method', 'constructor'].includes(d.kind)) continue;
      const ownerFactId = JSON.parse(d.metadata.value).ownerFactId;
      if (!classFactIds.has(ownerFactId)) continue;
      if (own.parameters.length > 64 || (own.typeParameters?.length || 0) > 16
        || own.typeParameters?.some(p => !identifier(p.name))) fail('javascript_class_relations_omitted');
      relationFact('js_callable_signature', own, { policy: 'javascript-class-members-1',
        declarationFactId: d.fact.id, ownerFactId,
        parameters: own.parameters.map(p => ({ name: directName(p.name) || '<binding>',
          pattern: !ts.isIdentifier(p.name), optional: !!p.questionToken, rest: !!p.dotDotDotToken,
          hasDefault: !!p.initializer, type: p.type ? typeExpr(p.type) : null })),
        returnType: own.type ? typeExpr(own.type) : null,
        typeParameters: (own.typeParameters || []).map(p => p.name.text) }, d.fact.symbol);
    }
    for (const c of classes) {
      if (!declarations.has(c)) continue;
      for (const h of c.heritageClauses || []) {
        if (h.token !== ts.SyntaxKind.ExtendsKeyword) continue;
        for (const t of h.types) {
          const b = unique(t.expression);
          const candidate = classTarget(b);
          const target = stable(b) && candidate && candidate !== c
            && b.node.end < t.getStart(sf) ? candidate : null;
          const kind = target ? 'local_class' : importedTarget(b, t.expression) ? 'import'
            : parameterKind(b, c, c) || 'unresolved';
          emit(c, c, t, 'extends', baseFacts.get(t)?.value || expr(t), b, kind,
            target, null, null, baseFacts.get(t), t.expression);
        }
      }
    }
    const stored = new WeakSet();
    function store(c, ctor, node, value, parts, depth = 0) {
      if (Buffer.byteLength(parts.join('.')) > 512) return;
      if (identifier(value)) {
        const b = unique(value);
        const kind = parameterKind(b, c, ctor);
        if (kind) {
          emit(c, ctor, node, 'stores_parameter', value.text, b, kind, null,
            parts.join('.'), b.node.type ? typeExpr(b.node.type) : null, null, value);
          stored.add(value);
        }
      } else if (value && ts.isObjectLiteralExpression(value) && depth < 4) {
        // Unknown keys/spreads/accessors can overwrite siblings. Skip the whole object.
        const keys = new Set();
        if (!value.properties.every(p => (ts.isPropertyAssignment(p) || ts.isShorthandPropertyAssignment(p))
          && identifier(p.name) && p.name.text !== '__proto__' && !p.objectAssignmentInitializer
          && !keys.has(p.name.text) && keys.add(p.name.text))) return;
        for (const p of value.properties) store(c, ctor, node,
          ts.isPropertyAssignment(p) ? p.initializer : p.name, [...parts, p.name.text], depth + 1);
      }
    }
    // Only direct method bodies are selected. Nested functions (including arrows)
    // and type/declaration/property-name positions never supply reference evidence.
    function context(n) {
      let own = null;
      for (let p = n.parent; p; p = p.parent) {
        if (ts.isTypeNode(p) || ts.isParameter(p) || ts.isDecorator(p)
          || ts.isComputedPropertyName(p) || ts.isHeritageClause(p)) return null;
        if (ts.isFunctionLike(p)) {
          if (own || !declarations.has(p)) return null;
          own = p;
        }
        if (ts.isClassDeclaration(p) || ts.isClassExpression(p)) {
          return own && declarations.has(p) && ['method', 'constructor'].includes(declarations.get(own).kind)
            ? { c: p, own } : null;
        }
      }
      return null;
    }
    for (const n of allNodes) {
      if (ts.isParameter(n) && ts.isConstructorDeclaration(n.parent) && declarations.has(n.parent)
        && n.modifiers?.some(m => [ts.SyntaxKind.PublicKeyword, ts.SyntaxKind.PrivateKeyword,
          ts.SyntaxKind.ProtectedKeyword, ts.SyntaxKind.ReadonlyKeyword].includes(m.kind))
        && identifier(n.name)) {
        const ctor = n.parent;
        const c = ctor.parent;
        const b = unique(n.name);
        const kind = declarations.has(c) && parameterKind(b, c, ctor);
        if (kind) emit(c, ctor, n, 'stores_parameter', n.name.text, b, kind, null,
          n.name.text, n.type ? typeExpr(n.type) : null, null, n.name);
      }
      if (!ts.isBinaryExpression(n) || n.operatorToken.kind !== ts.SyntaxKind.EqualsToken) continue;
      const ctx = context(n);
      if (!ctx || !ts.isConstructorDeclaration(ctx.own)) continue;
      if (ts.isPropertyAccessExpression(n.left) && !n.left.questionDotToken
        && n.left.expression.kind === ts.SyntaxKind.ThisKeyword && directName(n.left.name)
        && Buffer.byteLength(n.left.name.text) <= 512) {
        const cd = declarations.get(ctx.c);
        const od = declarations.get(ctx.own);
        relationFact('js_constructor_field', n, { policy: 'javascript-class-members-1',
          classFactId: cd.fact.id, ownerFactId: od.fact.id, memberName: n.left.name.text,
          visibility: ts.isPrivateIdentifier(n.left.name) ? 'private' : 'public', static: false,
          assignmentKind: 'direct-constructor-assignment', sourceType: null,
          supportingFactIds: [cd.fact.id, cd.metadata.id, od.fact.id, od.metadata.id] }, od.fact.symbol);
      }
      let lhs = n.left;
      const parts = [];
      while (ts.isPropertyAccessExpression(lhs) && !lhs.questionDotToken && identifier(lhs.name)) {
        parts.unshift(lhs.name.text); lhs = lhs.expression;
      }
      if (parts.length && lhs.kind === ts.SyntaxKind.ThisKeyword) store(ctx.c, ctx.own, n, n.right, parts);
    }
    const selected = new Map();
    for (const n of allNodes) {
      if (!identifier(n) || stored.has(n)) continue;
      const p = n.parent;
      if ((p.name === n && !ts.isShorthandPropertyAssignment(p)) || p.propertyName === n
        || ((ts.isBreakStatement(p) || ts.isContinueStatement(p) || ts.isLabeledStatement(p)) && p.label === n)) continue;
      const ctx = context(n);
      if (!ctx) continue;
      const b = unique(n);
      let kind = parameterKind(b, ctx.c, ctx.own);
      const creation = ts.isNewExpression(p) && p.expression === n;
      let target = null;
      if (creation && !kind && stable(b)) {
        const candidate = classTarget(b);
        if (candidate && b.node.end < n.getStart(sf)) {
          kind = 'local_class';
          target = candidate;
        } else if (importedTarget(b, n)) kind = 'import';
      }
      if (!kind || (!creation && kind !== 'factory_parameter')) continue;
      const relationship = creation ? 'creates' : 'uses_factory_binding';
      if (!selected.has(ctx.c)) selected.set(ctx.c, new Map());
      const byBinding = selected.get(ctx.c);
      if (!byBinding.has(b)) byBinding.set(b, new Set());
      if (byBinding.get(b).has(relationship)) continue;
      byBinding.get(b).add(relationship);
      emit(ctx.c, ctx.own, creation ? p : n, relationship, n.text, b, kind,
        target, null, null, null, n);
    }
  }
  try {
    classRelations();
  } catch (error) {
    if (error.extractorCode !== 'javascript_class_relations_omitted') throw error;
    relationFacts.length = 0;
    relationBytes = 0;
    diagnostic('javascript_class_relations_omitted', file);
  }
  fact('js_module_summary', sf, { ownerFactId: moduleFact.id,
    policy: 'selected-relative-classes-1', mode: moduleMode, importsSafe, exportsSafe,
    dependencies: [...dependencies].sort() });
  for (const f of structuralFacts) facts.push(f);
  for (const f of relationFacts) if (f.kind === 'js_class_scope') facts.push(f);
  for (const f of relationFacts) if (f.kind !== 'js_class_scope') facts.push(f);
  structuralUsage.bytes += structuralBytes;
  structuralUsage.facts += structuralFacts.length;
  relationUsage.bytes += relationBytes;
  relationUsage.facts += relationFacts.length;
  return facts;
}

function main(input) {
  const compilerFile = process.argv[1];
  const expectedSha256 = process.argv[2];
  if (!compilerFile || !path.isAbsolute(compilerFile)) fail('trusted_compiler_unavailable');
  if (!expectedSha256 || !/^[0-9a-fA-F]{64}$/.test(expectedSha256)) fail('compiler_changed');
  const maxCompiler = 32 * 1024 * 1024;
  let fd;
  let bytes;
  try {
    fd = fs.openSync(compilerFile, fs.constants.O_RDONLY | (fs.constants.O_NOFOLLOW || 0)
      | (fs.constants.O_NONBLOCK || 0));
    const stat = fs.fstatSync(fd);
    if (!stat.isFile() || stat.size > maxCompiler) fail('compiler_changed');
    const buffer = Buffer.alloc(maxCompiler + 1);
    let length = 0;
    while (length < buffer.length) {
      const read = fs.readSync(fd, buffer, length, buffer.length - length, null);
      if (!read) break;
      length += read;
    }
    if (length > maxCompiler) fail('compiler_changed');
    bytes = buffer.subarray(0, length);
  } catch (_) {
    fail('compiler_changed');
  } finally {
    if (fd !== undefined) fs.closeSync(fd);
  }
  result.typescriptSha256 = sha(bytes);
  if (result.typescriptSha256 !== expectedSha256.toLowerCase()) fail('compiler_changed');
  // Compile precisely the bytes hashed above, without target package/config lookup.
  const compiler = new Module(compilerFile, null);
  compiler.filename = compilerFile;
  compiler.paths = [];
  compiler.require = name => {
    if (!Module.isBuiltin(name)) fail('compiler_nonbuiltin_dependency');
    return require(name);
  };
  compiler._compile(bytes.toString('utf8'), compilerFile);
  ts = compiler.exports;
  if (typeof ts.version !== 'string' || typeof ts.createSourceFile !== 'function') fail('trusted_compiler_invalid');
  result.typescriptVersion = ts.version;
  const payload = JSON.parse(input);
  if (!payload || !Array.isArray(payload.sources) || payload.sources.length > LIMIT.files) fail('invalid_input');
  let total = 0;
  const seen = new Set();
  for (const source of payload.sources) {
    if (result.facts.filter(f => !optionalFact(f)).length >= LIMIT.facts) {
      result.complete = false; diagnostic('resource_limit', ''); break;
    }
    if (!source || typeof source.path !== 'string' || typeof source.module !== 'string'
      || typeof source.hex !== 'string' || !source.path || source.path.length > 4096
      || source.module.length > 4096 || path.isAbsolute(source.path) || source.path.includes('\\')
      || source.path.split('/').some(x => !x || x === '..' || x === '.') || seen.has(source.path)
      || source.hex.length > LIMIT.bytes * 2 || !/^(?:[0-9a-fA-F]{2})*$/.test(source.hex)) fail('invalid_input');
    seen.add(source.path);
    total += source.hex.length / 2;
    if (total > LIMIT.total) fail('resource_limit');
    const start = result.facts.length;
    try {
      const text = new TextDecoder('utf-8', { fatal: true, ignoreBOM: true }).decode(Buffer.from(source.hex, 'hex'));
      for (const f of extract(source, text)) result.facts.push(f);
    } catch (error) {
      result.facts.length = start;
      // Module identity survives failed decoding, traversal, or compiler exceptions.
      result.facts.push({ id: `module_${sha(source.path)}`, kind: 'module', path: source.path,
        line: 1, endLine: 1, symbol: source.module, value: source.module });
      result.complete = false;
      diagnostic(error.extractorCode || 'javascript_source_unavailable', source.path);
    }
  }
}

let inputBytes = 0;
const chunks = [];
process.stdin.on('data', chunk => {
  inputBytes += chunk.length;
  if (inputBytes <= LIMIT.input) chunks.push(chunk);
});
process.stdin.on('end', () => {
  try {
    if (inputBytes > LIMIT.input) fail('resource_limit');
    main(Buffer.concat(chunks).toString('utf8'));
  } catch (error) {
    result.complete = false;
    diagnostic(error.extractorCode || 'javascript_extractor_failed', '');
  }
  const canonical = new Map();
  for (const f of result.facts) {
    if (canonical.has(f.id) && JSON.stringify(canonical.get(f.id)) !== JSON.stringify(f)) {
      canonical.clear(); result.complete = false;
      diagnostic('javascript_conflicting_fact_id', ''); break;
    }
    canonical.set(f.id, f);
  }
  result.facts = [...canonical.values()];
  function dropOptionalFacts() {
    const relationsDropped = result.facts.some(f => relationKinds.has(f.kind));
    const structuralDropped = result.facts.some(f => optionalFact(f) && !relationKinds.has(f.kind));
    const baseline = result.facts.filter(f => !optionalFact(f));
    if (baseline.length === result.facts.length) return;
    result.facts = baseline;
    if (structuralDropped) diagnostic('javascript_structural_facts_omitted', '');
    if (relationsDropped) diagnostic('javascript_class_relations_omitted', '');
  }
  if (result.facts.length > LIMIT.facts) dropOptionalFacts();
  let output = serializeResult();
  if (Buffer.byteLength(output) > LIMIT.output) {
    dropOptionalFacts();
    output = serializeResult();
  }
  if (Buffer.byteLength(output) > LIMIT.output) {
    result.facts = []; result.complete = false;
    diagnostic('resource_limit', '');
    output = serializeResult();
  }
  process.stdout.write(output);
});
