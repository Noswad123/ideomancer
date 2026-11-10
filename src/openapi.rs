//! Local contract metadata only. No fetching, reference expansion, or application execution.
use crate::catalog::Item;
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::Path;

pub const MAX_BYTES: usize = 4 * 1024 * 1024;
const MAX_OPERATIONS: usize = 4000;
const MAX_NODES: usize = 100_000;
const MAX_SCALAR_BYTES: usize = MAX_BYTES;

// Enforce bounds during deserialization, not after YAML aliases have expanded.
struct Bounded<'a> {
    depth: usize,
    remaining: &'a mut usize,
    scalar_bytes: &'a mut usize,
    key: bool,
}

impl<'de> serde::de::DeserializeSeed<'de> for Bounded<'_> {
    type Value = Value;
    fn deserialize<D: serde::Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        use serde::de::Error;
        if self.depth > 48 || *self.remaining == 0 {
            return Err(D::Error::custom(
                "OpenAPI structural depth/node limit exceeded",
            ));
        }
        *self.remaining -= 1;
        if self.key {
            deserializer.deserialize_str(self)
        } else {
            deserializer.deserialize_any(self)
        }
    }
}

impl Bounded<'_> {
    fn charge_scalar<E: serde::de::Error>(&mut self, bytes: usize) -> Result<(), E> {
        *self.scalar_bytes = self
            .scalar_bytes
            .checked_sub(bytes)
            .ok_or_else(|| E::custom("OpenAPI expanded scalar/key byte limit exceeded"))?;
        Ok(())
    }
}

impl<'de> serde::de::Visitor<'de> for Bounded<'_> {
    type Value = Value;
    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("bounded JSON/YAML contract data")
    }
    fn visit_bool<E: serde::de::Error>(self, v: bool) -> Result<Value, E> {
        Ok(json!(v))
    }
    fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<Value, E> {
        Ok(json!(v))
    }
    fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Value, E> {
        Ok(json!(v))
    }
    fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<Value, E> {
        serde_json::Number::from_f64(v)
            .map(Value::Number)
            .ok_or_else(|| E::custom("Non-finite OpenAPI number"))
    }
    fn visit_str<E: serde::de::Error>(mut self, v: &str) -> Result<Value, E> {
        self.charge_scalar::<E>(v.len())?;
        Ok(Value::String(v.to_owned()))
    }
    fn visit_string<E: serde::de::Error>(mut self, v: String) -> Result<Value, E> {
        self.charge_scalar::<E>(v.len())?;
        Ok(Value::String(v))
    }
    fn visit_unit<E: serde::de::Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_none<E: serde::de::Error>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }
    fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut seq: A) -> Result<Value, A::Error> {
        let mut out = Vec::new();
        while let Some(v) = seq.next_element_seed(Bounded {
            depth: self.depth + 1,
            remaining: self.remaining,
            scalar_bytes: self.scalar_bytes,
            key: false,
        })? {
            out.push(v);
        }
        Ok(Value::Array(out))
    }
    fn visit_map<A: serde::de::MapAccess<'de>>(self, mut map: A) -> Result<Value, A::Error> {
        use serde::de::Error;
        let mut out = serde_json::Map::new();
        while let Some(key) = map.next_key_seed(Bounded {
            depth: self.depth + 1,
            remaining: self.remaining,
            scalar_bytes: self.scalar_bytes,
            key: true,
        })? {
            let Value::String(key) = key else {
                return Err(A::Error::custom("OpenAPI object key must be a string"));
            };
            if out.contains_key(&key) {
                return Err(A::Error::custom("Duplicate OpenAPI object key"));
            }
            let v = map.next_value_seed(Bounded {
                depth: self.depth + 1,
                remaining: self.remaining,
                scalar_bytes: self.scalar_bytes,
                key: false,
            })?;
            out.insert(key, v);
        }
        Ok(Value::Object(out))
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Document {
    pub path: String,
    pub sha256: String,
    pub version: String,
    pub operations: Vec<Operation>,
    pub security_schemes: Value,
    pub boundaries: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Operation {
    pub method: String,
    pub path: String,
    pub pointer: String,
    pub operation_id: Option<String>,
    pub summary: Option<String>,
    pub tags: Vec<String>,
    pub parameters: Vec<Value>,
    pub request: Value,
    pub responses: Value,
    pub security: Value,
    pub security_basis: String,
    pub servers: Value,
    pub server_basis: String,
    pub boundaries: Vec<String>,
}

/// Open every component with NOFOLLOW, including the final regular file. On
/// unsupported platforms there is deliberately no racy pathname fallback.
#[cfg(unix)]
pub fn load(path: &Path) -> Result<Document, String> {
    use rustix::fs::{open, openat, Mode, OFlags};
    use std::path::Component;
    let absolute = std::env::current_dir()
        .map_err(|e| e.to_string())?
        .join(path);
    let mut file = std::fs::File::from(
        open(
            "/",
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|e| e.to_string())?,
    );
    let parts: Vec<_> = absolute
        .components()
        .filter(|c| *c != Component::RootDir && *c != Component::CurDir)
        .collect();
    if parts.is_empty() || parts.iter().any(|c| !matches!(c, Component::Normal(_))) {
        return Err("OpenAPI path must be a local file without parent traversal".into());
    }
    for (i, part) in parts.iter().enumerate() {
        let mut flags = OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC;
        if i + 1 < parts.len() {
            flags |= OFlags::DIRECTORY;
        }
        file = std::fs::File::from(
            openat(&file, part.as_os_str(), flags, Mode::empty())
                .map_err(|e| format!("OpenAPI requires a regular non-symlink local file: {e}"))?,
        );
    }
    let meta = file.metadata().map_err(|e| e.to_string())?;
    if !meta.is_file() || meta.len() > MAX_BYTES as u64 {
        return Err("OpenAPI requires a regular file no larger than 4 MiB".into());
    }
    let mut bytes = Vec::new();
    file.take((MAX_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    parse(&absolute.to_string_lossy(), &bytes)
}

#[cfg(not(unix))]
pub fn load(_path: &Path) -> Result<Document, String> {
    Err("OpenAPI loading requires Unix no-follow filesystem support".into())
}

fn text(v: &Value) -> Option<String> {
    v.as_str().map(|s| s.chars().take(2048).collect())
}

fn token(s: &str) -> String {
    s.replace('~', "~0").replace('/', "~1")
}

fn inspect(v: &Value, depth: usize, remaining: &mut usize) -> Result<(), String> {
    if depth > 48 || *remaining == 0 {
        return Err("OpenAPI structural depth/node limit exceeded".into());
    }
    *remaining -= 1;
    match v {
        Value::Object(map) => {
            if let Some(r) = map.get("$ref") {
                let r = r.as_str().ok_or("OpenAPI $ref must be a string")?;
                if !r.starts_with("#/") {
                    return Err(
                        "External or unsupported OpenAPI $ref rejected; no references are fetched"
                            .into(),
                    );
                }
            }
            for child in map.values() {
                inspect(child, depth + 1, remaining)?;
            }
        }
        Value::Array(a) => {
            for child in a {
                inspect(child, depth + 1, remaining)?;
            }
        }
        _ => {}
    }
    Ok(())
}

// A bounded shape summary, not a copy of user payloads or recursively expanded schemas.
fn schema(v: &Value, root: &Value, depth: usize, budget: &mut usize) -> Value {
    if *budget == 0 || depth > 5 {
        return json!({"boundary":"schema summary limit"});
    }
    *budget -= 1;
    if let Some(r) = v.get("$ref").and_then(Value::as_str) {
        return json!({"$ref":r,"status": if root.pointer(&r[1..]).is_some() {"local reference; not expanded"} else {"unresolved local reference; no fetch"}});
    }
    if v.is_boolean() {
        return v.clone();
    }
    let mut out = serde_json::Map::new();
    for k in ["type", "format", "nullable", "readOnly", "writeOnly"] {
        if let Some(value) = v.get(k) {
            if value.is_boolean() || value.is_string() {
                out.insert(k.into(), value.clone());
            }
        }
    }
    if let Some(types) = v["type"].as_array() {
        out.insert(
            "type".into(),
            json!(types.iter().filter_map(text).take(16).collect::<Vec<_>>()),
        );
    }
    if let Some(props) = v["properties"].as_object() {
        let mut p = serde_json::Map::new();
        for (k, v) in props.iter().take(64) {
            p.insert(k.clone(), schema(v, root, depth + 1, budget));
        }
        out.insert("properties".into(), Value::Object(p));
        if props.len() > 64 {
            out.insert("boundary".into(), json!("property limit"));
        }
    }
    for k in ["items", "additionalProperties", "not"] {
        if let Some(v) = v.get(k) {
            out.insert(k.into(), schema(v, root, depth + 1, budget));
        }
    }
    for k in ["allOf", "oneOf", "anyOf"] {
        if let Some(a) = v[k].as_array() {
            out.insert(
                k.into(),
                json!(a
                    .iter()
                    .take(16)
                    .map(|v| schema(v, root, depth + 1, budget))
                    .collect::<Vec<_>>()),
            );
        }
    }
    if let Some(a) = v["required"].as_array() {
        out.insert(
            "required".into(),
            json!(a.iter().filter_map(text).take(64).collect::<Vec<_>>()),
        );
    }
    Value::Object(out)
}

fn content(v: &Value, root: &Value) -> Value {
    let mut out = serde_json::Map::new();
    if let Some(m) = v.as_object() {
        for (media, v) in m.iter().take(16) {
            out.insert(media.clone(), schema(&v["schema"], root, 0, &mut 256));
        }
    }
    Value::Object(out)
}

fn ref_boundary(v: &Value) -> Option<Value> {
    v.get("$ref")
        .map(|r| json!({"$ref":r,"boundary":"reference object unresolved; no expansion or fetch"}))
}

fn parameter(v: &Value, root: &Value) -> Value {
    if let Some(r) = ref_boundary(v) {
        return r;
    }
    if !v["name"].is_string()
        || !matches!(
            v["in"].as_str(),
            Some("query" | "header" | "path" | "cookie" | "body" | "formData")
        )
    {
        return json!({"boundary":"malformed parameter object; effective parameters incomplete"});
    }
    json!({"name":text(&v["name"]),"in":text(&v["in"]),"required":v["required"].as_bool(),
        "schema":schema(v.get("schema").unwrap_or(v),root,0,&mut 256),"content":content(&v["content"],root)})
}

fn security(v: Option<&Value>) -> Value {
    let Some(a) = v.and_then(Value::as_array) else {
        return json!({"boundary":"security absent or malformed; not verified"});
    };
    json!(a
        .iter()
        .take(32)
        .map(|requirement| {
            let mut out = serde_json::Map::new();
            if let Some(m) = requirement.as_object() {
                for (name, scopes) in m.iter().take(32) {
                    let Some(a) = scopes.as_array().filter(|a| a.iter().all(Value::is_string))
                    else {
                        return json!({"boundary":"malformed security scopes; not verified"});
                    };
                    out.insert(
                        name.clone(),
                        json!(a.iter().filter_map(text).take(64).collect::<Vec<_>>()),
                    );
                }
            } else {
                return json!({"boundary":"malformed security requirement; not verified"});
            }
            Value::Object(out)
        })
        .collect::<Vec<_>>())
}

fn servers(v: &Value) -> Value {
    json!(v
        .as_array()
        .map(|a| a
            .iter()
            .take(32)
            .map(|s| {
                if s.get("$ref").is_some() {
                    return json!({"boundary":"server reference unresolved"});
                }
                // Variables are never substituted; their defaults can be credentials.
                json!({"url":text(&s["url"]),"variables":"not substituted; defaults omitted"})
            })
            .collect::<Vec<_>>())
        .unwrap_or_default())
}

fn parse(path: &str, bytes: &[u8]) -> Result<Document, String> {
    if bytes.len() > MAX_BYTES {
        return Err("OpenAPI exceeds the 4 MiB limit".into());
    }
    use serde::de::DeserializeSeed;
    let mut budget = MAX_NODES;
    let mut scalar_bytes = MAX_SCALAR_BYTES;
    let bounded = Bounded {
        depth: 0,
        remaining: &mut budget,
        scalar_bytes: &mut scalar_bytes,
        key: false,
    };
    let root: Value = if bytes.iter().find(|b| !b.is_ascii_whitespace()) == Some(&b'{') {
        let mut deserializer = serde_json::Deserializer::from_slice(bytes);
        let value = bounded
            .deserialize(&mut deserializer)
            .map_err(|e| format!("Invalid OpenAPI JSON: {e}"))?;
        deserializer
            .end()
            .map_err(|e| format!("Invalid OpenAPI JSON: {e}"))?;
        value
    } else {
        bounded
            .deserialize(serde_yaml::Deserializer::from_slice(bytes))
            .map_err(|e| format!("Invalid OpenAPI YAML: {e}"))?
    };
    let mut remaining = MAX_NODES;
    inspect(&root, 0, &mut remaining)?;
    let version = root["openapi"]
        .as_str()
        .filter(|v| v.starts_with("3.0.") || v.starts_with("3.1."))
        .or_else(|| root["swagger"].as_str().filter(|v| *v == "2.0"))
        .ok_or("Expected OpenAPI 3.0/3.1 or Swagger 2.0")?
        .to_string();
    let swagger = version == "2.0";
    let paths = root["paths"]
        .as_object()
        .ok_or("OpenAPI paths must be an object")?;
    let mut operations = Vec::new();
    let mut boundaries = vec!["Local declared contract only; server reachability and security are not verified. Examples, defaults and payload values omitted.".into()];
    for (path, entry) in paths {
        if !path.starts_with('/') {
            return Err("OpenAPI operation path must start with /".into());
        }
        if entry.get("$ref").is_some() {
            boundaries.push(format!(
                "Path item {path}: reference object unresolved; sibling operations ignored"
            ));
            continue;
        }
        for method in [
            "get", "put", "post", "delete", "options", "head", "patch", "trace",
        ] {
            let Some(op) = entry.get(method) else {
                continue;
            };
            if !op.is_object() {
                return Err("OpenAPI operation must be an object".into());
            }
            if operations.len() == MAX_OPERATIONS {
                return Err("OpenAPI exceeds the 4000 operation limit".into());
            }
            let mut b = Vec::new();
            if op.get("$ref").is_some() {
                boundaries.push(format!("{method} {path}: operation reference unresolved"));
                continue;
            }
            let mut parameters = Vec::new();
            for owner in [entry, op] {
                if let Some(p) = owner.get("parameters") {
                    let a = p.as_array().ok_or("OpenAPI parameters must be an array")?;
                    if a.len() > 128 {
                        return Err("OpenAPI parameter limit exceeded".into());
                    }
                    for p in a {
                        let p = parameter(p, &root);
                        if p.get("boundary").is_some() {
                            b.push(
                                "Parameter reference unresolved; effective parameters incomplete"
                                    .into(),
                            );
                        } else {
                            parameters.retain(|old: &Value| {
                                old["name"] != p["name"] || old["in"] != p["in"]
                            });
                        }
                        parameters.push(p);
                    }
                }
            }
            let request = if let Some(r) = op.get("requestBody") {
                ref_boundary(r).unwrap_or_else(|| json!({"required":r["required"].as_bool(),"content":content(&r["content"],&root)}))
            } else {
                Value::Null
            };
            let mut responses = serde_json::Map::new();
            if let Some(m) = op["responses"].as_object() {
                if m.len() > 64 {
                    return Err("OpenAPI response limit exceeded".into());
                }
                for (code, r) in m {
                    responses.insert(code.clone(), ref_boundary(r).unwrap_or_else(|| json!({"schema":schema(&r["schema"],&root,0,&mut 256),"content":content(&r["content"],&root)})));
                }
            }
            let (server, server_basis) = if swagger {
                (
                    json!({"host":text(&root["host"]),"basePath":text(&root["basePath"]),"schemes":op.get("schemes").or_else(||root.get("schemes")).and_then(Value::as_array).map(|a|a.iter().filter_map(text).take(16).collect::<Vec<_>>()),"note":"declared components only; no URL inferred"}),
                    "swagger declared components",
                )
            } else if op.get("servers").is_some() {
                (servers(&op["servers"]), "operation")
            } else if entry.get("servers").is_some() {
                (servers(&entry["servers"]), "path")
            } else {
                (
                    servers(&root["servers"]),
                    "root (absent means unknown; no default inferred)",
                )
            };
            operations.push(Operation {
                method: method.to_uppercase(),
                path: path.clone(),
                pointer: format!("/paths/{}/{method}", token(path)),
                operation_id: text(&op["operationId"]),
                summary: text(&op["summary"]),
                tags: op["tags"]
                    .as_array()
                    .map(|a| a.iter().filter_map(text).take(32).collect())
                    .unwrap_or_default(),
                parameters,
                request,
                responses: Value::Object(responses),
                security: security(op.get("security").or_else(|| root.get("security"))),
                security_basis: if op.get("security").is_some() {
                    "operation; declared, not verified"
                } else {
                    "root; declared, not verified"
                }
                .into(),
                servers: server,
                server_basis: server_basis.into(),
                boundaries: b,
            });
        }
    }
    operations.sort_by(|a, b| (&a.path, &a.method).cmp(&(&b.path, &b.method)));
    let mut schemes = serde_json::Map::new();
    let defs = if swagger {
        &root["securityDefinitions"]
    } else {
        &root["components"]["securitySchemes"]
    };
    if let Some(m) = defs.as_object() {
        for (name, v) in m.iter().take(64) {
            let mut out = serde_json::Map::new();
            for k in [
                "type",
                "scheme",
                "bearerFormat",
                "name",
                "in",
                "flow",
                "authorizationUrl",
                "tokenUrl",
                "openIdConnectUrl",
            ] {
                if let Some(s) = text(&v[k]) {
                    out.insert(k.into(), json!(s));
                }
            }
            if let Some(r) = ref_boundary(v) {
                schemes.insert(name.clone(), r);
            } else {
                if let Some(flows) = v["flows"].as_object() {
                    let mut normalized = serde_json::Map::new();
                    for flow in [
                        "implicit",
                        "password",
                        "clientCredentials",
                        "authorizationCode",
                    ] {
                        if let Some(f) = flows.get(flow) {
                            normalized.insert(flow.into(),json!({"authorizationUrl":text(&f["authorizationUrl"]),
                                "tokenUrl":text(&f["tokenUrl"]),"refreshUrl":text(&f["refreshUrl"]),
                                "scopes":f["scopes"].as_object().map(|s|s.keys().take(64).collect::<Vec<_>>())}));
                        }
                    }
                    out.insert("flows".into(), Value::Object(normalized));
                }
                if let Some(scopes) = v["scopes"].as_object() {
                    out.insert(
                        "scopes".into(),
                        json!(scopes.keys().take(64).collect::<Vec<_>>()),
                    );
                }
                schemes.insert(name.clone(), Value::Object(out));
            }
        }
    }
    Ok(Document {
        path: path.into(),
        sha256: format!("{:x}", Sha256::digest(bytes)),
        version,
        operations,
        security_schemes: Value::Object(schemes),
        boundaries,
    })
}

/// Candidate matching is intentionally literal: no router prefixes, parameter
/// renaming, generated operation IDs or inferred server URLs.
pub fn match_item(document: &Document, item: &Item, handler_name_unique: bool) -> Value {
    let mut basis = "exact method/path";
    let mut candidates: Vec<_> = document
        .operations
        .iter()
        .filter(|o| {
            item.method.as_deref() == Some(&o.method) && item.route_path.as_deref() == Some(&o.path)
        })
        .collect();
    if item.path_is_fragment {
        basis = "method and suffix fragment candidate; prefix/activation unverified";
        if let (Some(method), Some(path)) = (&item.method, &item.route_path) {
            if path.starts_with('/') {
                candidates = document
                    .operations
                    .iter()
                    .filter(|o| &o.method == method && o.path.ends_with(path))
                    .collect();
            }
        }
    }
    if candidates.is_empty() && handler_name_unique {
        basis = "unique operationId/handler name candidate; route/activation unverified";
        let name = item.symbol.rsplit('.').next().unwrap_or("");
        candidates = document
            .operations
            .iter()
            .filter(|o| {
                o.operation_id.as_deref() == Some(name)
                    && item.method.as_ref().is_none_or(|m| m == &o.method)
            })
            .collect();
    }
    json!({"status":match candidates.len() {0=>"unmatched",1=>"candidate",_=>"ambiguous"},"basis":basis,
        "candidates":candidates,"provenance":{"path":document.path,"sha256":document.sha256},
        "boundary":"Contract association is not evidence of runtime registration, reachability or authentication."})
}

// Strict, bounded declaration projection, not a runtime route parser.
fn project_converters(path: &str) -> Result<(String, Vec<Value>), &'static str> {
    if path.len() > 2048 {
        return Err("route exceeds projection byte limit");
    }
    let mut out = String::new();
    let mut parameters = Vec::new();
    let mut rest = path;
    while !rest.is_empty() {
        let end = rest.find(['{', '}']).unwrap_or(rest.len());
        out.push_str(&rest[..end]);
        rest = &rest[end..];
        if rest.is_empty() {
            break;
        }
        if !rest.starts_with('{') {
            return Err("malformed braces");
        }
        let close = rest.find('}').ok_or("truncated placeholder")?;
        let body = &rest[1..close];
        let mut parts = body.split(':');
        let name = parts.next().unwrap_or("");
        if name.is_empty()
            || !name
                .bytes()
                .enumerate()
                .all(|(i, b)| b == b'_' || b.is_ascii_alphabetic() || i > 0 && b.is_ascii_digit())
        {
            return Err("malformed parameter name or nested braces");
        }
        let converter = parts.next();
        if parts.next().is_some() {
            return Err("repeated colon");
        }
        if let Some(converter) = converter {
            if !matches!(converter, "str" | "path" | "int" | "float" | "uuid") {
                return Err("unsupported converter; entire candidate projection rejected");
            }
        }
        if parameters.len() == 64 {
            return Err("parameter inventory limit");
        }
        parameters.push(json!({"parameter":name,"converter":converter}));
        out.push('{');
        out.push_str(name);
        out.push('}');
        rest = &rest[close + 1..];
    }
    Ok((out, parameters))
}

/// A source-backed prefix is a suffix convention, not proof of an outer mount.
/// Supported contradictory context must never fall back to a generic handler ID.
pub fn match_item_with_context(
    document: &Document,
    item: &Item,
    handler_name_unique: bool,
    context: &crate::router_context::Context,
) -> Value {
    if context.status == "budget-unknown" {
        return json!({"status":"unknown","basis":"router context proof budget boundary; association not narrowed","candidates":[],
            "provenance":{"path":document.path,"sha256":document.sha256}});
    }
    if context.status != "supported" || context.candidates.is_empty() {
        return match_item(document, item, handler_name_unique);
    }
    let eligible = context.converter_projection.status == "eligible";
    let projections: Vec<_> = context.candidates.iter().map(|c| {
        if !eligible {
            return json!({"originalPath":c.composed_path,"projectedPath":null,"parameters":[],"status":"blocked","reason":"context converter eligibility not established"});
        }
        match project_converters(&c.composed_path) {
            Ok((path, parameters)) => {
                let changed = path != c.composed_path;
                json!({"originalPath":c.composed_path,"projectedPath":path,"parameters":parameters,
                    "status":if changed {"projected"} else {"unchanged"},
                    "reason":if changed {"known built-in declaration projection"} else {"no converter projection needed"}})
            },
            Err(reason) => json!({"originalPath":c.composed_path,"projectedPath":null,"parameters":[],"status":"rejected","reason":reason}),
        }
    }).collect();
    let candidates: Vec<_> = document
        .operations
        .iter()
        .filter(|op| {
            item.method.as_deref() == Some(&op.method)
                && context
                    .candidates
                    .iter()
                    .zip(&projections)
                    .any(|(c, projection)| {
                        !c.composed_path.is_empty()
                            && [
                                Some(c.composed_path.as_str()),
                                projection["projectedPath"].as_str(),
                            ]
                            .into_iter()
                            .flatten()
                            .any(|path| {
                                if c.full_path_proven {
                                    op.path == path
                                } else {
                                    op.path.ends_with(path)
                                }
                            })
                    })
        })
        .collect();
    let basis = if projections.iter().any(|p| p["status"] == "projected") {
        "method and source-backed prefix/fragment suffix convention with literal and FastAPI/Starlette built-in converter projection candidates; outer mount and activation UNKNOWN"
    } else {
        "method and source-backed composed prefix/fragment suffix convention candidate; outer mount and activation UNKNOWN"
    };
    json!({"status":match candidates.len() {0=>"context-mismatch",1=>"candidate",_=>"ambiguous"},
        "basis":basis,
        "candidates":candidates,"provenance":{"path":document.path,"sha256":document.sha256},
        "converterProjections":projections,"converterProjectionContext":{"eligibility":context.converter_projection,"evidenceIds":context.evidence_ids,"sourceRefs":context.source_refs},
        "boundary":"Source convention and declared contract projection only, not runtime path, parameter type, routing, registration or activation proof. Hidden runtime converter modifications remain UNKNOWN."})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_projection_is_strict_bounded_and_preserves_literal_bytes() {
        for converter in ["str", "path", "int", "float", "uuid"] {
            let original = format!("/static:colon/%2F/{{Keep_Name:{converter}}}/{{second}}/");
            let (path, inventory) = project_converters(&original).unwrap();
            assert_eq!(path, "/static:colon/%2F/{Keep_Name}/{second}/");
            assert_eq!(
                inventory[0],
                json!({"parameter":"Keep_Name","converter":converter})
            );
            assert_eq!(inventory[1]["parameter"], "second");
        }
        assert_eq!(
            project_converters("/{a:int}/{B:uuid}").unwrap().0,
            "/{a}/{B}"
        );
        for path in [
            "/{a:path}/{b:custom}",
            "/{a:custom}/{b:int}",
            "/{{a:path}}",
            "/{a:path:int}",
            "/{a:path",
            "/a}",
            "/{:path}",
            "/{a:}",
            "/{a/b:path}",
        ] {
            assert!(project_converters(path).is_err(), "{path}");
        }
        assert!(project_converters(&format!("/{}{{a:path}}", "x".repeat(2048))).is_err());
    }

    #[test]
    fn converter_matching_unions_candidates_without_mutating_source() {
        let source =
            crate::router_context::tests::DIRECT.replace("'/items'", "'/by-name/{name:path}'");
        let Some((e, _, item)) = crate::router_context::tests::extract(&source) else {
            return;
        };
        let mut context = crate::router_context::derive(&e, &item);
        let original_item = serde_json::to_value(&item).unwrap();
        let original_context = serde_json::to_value(&context).unwrap();
        let doc = parse("synthetic.json", br#"{"openapi":"3.1.0","paths":{"/caps/by-name/{name}":{"get":{}},"/caps/by-name/{name:path}":{"get":{}},"/outer/caps/by-name/{name}":{"get":{}}}}"#).unwrap();
        let matched = match_item_with_context(&doc, &item, true, &context);
        assert_eq!(matched["status"], "ambiguous");
        assert_eq!(matched["candidates"].as_array().unwrap().len(), 3);
        assert_eq!(
            matched["converterProjections"][0]["projectedPath"],
            "/caps/by-name/{name}"
        );
        assert_eq!(serde_json::to_value(&item).unwrap(), original_item);
        assert_eq!(serde_json::to_value(&context).unwrap(), original_context);
        assert!(!context.candidates[0].full_path_proven);
        context.candidates.push(crate::router_context::Candidate {
            prefix: "/outer/caps".into(),
            composed_path: "/outer/caps/by-name/{name:path}".into(),
            full_path_proven: false,
        });
        assert_eq!(
            match_item_with_context(&doc, &item, true, &context)["candidates"]
                .as_array()
                .unwrap()
                .len(),
            3
        );
        context.converter_projection = Default::default();
        assert_eq!(
            match_item_with_context(&doc, &item, true, &context)["candidates"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            match_item(&doc, &item, false)["candidates"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        context.status = "budget-unknown".into();
        assert_eq!(
            match_item_with_context(&doc, &item, true, &context)["status"],
            "unknown"
        );
        context.status = "supported".into();
        context.candidates[0].composed_path = "/caps/by-name/".into();
        context.candidates.truncate(1);
        assert_eq!(
            match_item_with_context(&doc, &item, true, &context)["status"],
            "context-mismatch"
        );
    }

    #[test]
    fn versions_overrides_redaction_and_local_refs() {
        for version in ["3.0.3", "3.1.0"] {
            let bytes = json!({"openapi":version,"servers":[{"url":"https://root"}],"security":[{"key":[]}],"paths":{"/x":{"parameters":[{"$ref":"#/components/parameters/P"}],"get":{"security":[],"servers":[{"url":"/local","variables":{"token":{"default":"SECRET"}}}],"requestBody":{"content":{"application/json":{"schema":{"$ref":"#/components/schemas/S"},"example":"SECRET"}}},"responses":{"200":{"content":{"application/json":{"schema":{"type":"string","default":"SECRET"}}}}}}}},"components":{"schemas":{"S":{"type":"object"}}}}).to_string();
            let d = parse("local.json", bytes.as_bytes()).unwrap();
            let o = &d.operations[0];
            assert_eq!(o.security, json!([]));
            assert_eq!(o.server_basis, "operation");
            assert_eq!(o.servers[0]["url"], "/local");
            assert!(!o.boundaries.is_empty());
            assert!(o.request.to_string().contains("not expanded"));
            assert!(!serde_json::to_string(&d).unwrap().contains("SECRET"));
            assert_eq!(d.sha256.len(), 64);
        }
        let d = parse("swagger.yaml",b"swagger: '2.0'\nhost: example.test\nbasePath: /api\npaths:\n  /x:\n    post:\n      parameters:\n        - name: body\n          in: body\n          schema:\n            $ref: '#/definitions/Missing'\n      responses:\n        '200':\n          schema:\n            type: string\n").unwrap();
        assert_eq!(d.operations[0].servers["basePath"], "/api");
        assert!(d.operations[0].parameters[0]
            .to_string()
            .contains("unresolved local"));
    }

    #[test]
    fn rejects_external_refs_limits_and_path_ref_siblings() {
        for r in ["https://bad/spec.json#/x", "other.yaml#/x", "#anchor"] {
            let bytes =
                json!({"openapi":"3.1.0","paths":{},"components":{"schemas":{"S":{"$ref":r}}}})
                    .to_string();
            assert!(parse("x", bytes.as_bytes()).is_err());
        }
        assert!(parse("x", &vec![b' '; MAX_BYTES + 1]).is_err());
        assert!(parse("x", b"openapi: 4.0.0\npaths: {}").is_err());
        let d = parse(
            "x",
            br##"{"openapi":"3.1.0","paths":{"/x":{"$ref":"#/missing","get":{}}}}"##,
        )
        .unwrap();
        assert!(d.operations.is_empty());
        assert!(d.boundaries.iter().any(|b| b.contains("sibling")));
    }

    #[test]
    fn inherited_security_path_servers_parameter_override_and_limits() {
        let d = parse("x",br#"{"openapi":"3.1.0","servers":[{"url":"/root"}],"security":[{"oauth":["read"]}],"components":{"securitySchemes":{"oauth":{"type":"oauth2","flows":{"clientCredentials":{"tokenUrl":"/token","scopes":{"read":"SECRET_DESCRIPTION"}}}}}},"paths":{"/x":{"servers":[{"url":"/path"}],"parameters":[{"name":"q","in":"query","schema":{"type":"string"}}],"get":{"parameters":[{"name":"q","in":"query","schema":{"type":"integer"}}],"responses":{}}}}}"#).unwrap();
        let o = &d.operations[0];
        assert_eq!(o.security, json!([{"oauth":["read"]}]));
        assert_eq!(o.server_basis, "path");
        assert_eq!(o.servers[0]["url"], "/path");
        assert_eq!(o.parameters.len(), 1);
        assert_eq!(o.parameters[0]["schema"]["type"], "integer");
        assert_eq!(
            d.security_schemes["oauth"]["flows"]["clientCredentials"]["scopes"],
            json!(["read"])
        );
        assert!(!serde_json::to_string(&d)
            .unwrap()
            .contains("SECRET_DESCRIPTION"));
        assert!(security(Some(&json!(["bad"])))[0].get("boundary").is_some());
        let paths: serde_json::Map<String, Value> = (0..=MAX_OPERATIONS)
            .map(|i| (format!("/{i}"), json!({"get":{"responses":{}}})))
            .collect();
        assert!(parse(
            "x",
            json!({"openapi":"3.1.0","paths":paths})
                .to_string()
                .as_bytes()
        )
        .unwrap_err()
        .contains("operation limit"));
        let mut deep = json!({});
        for _ in 0..50 {
            deep = json!({"nested":deep});
        }
        assert!(inspect(&deep, 0, &mut 1000).is_err());
    }

    #[test]
    fn yaml_alias_expansion_is_bounded_before_materialization() {
        let mut yaml = String::from("openapi: 3.1.0\npaths: {}\na: &a [1,1,1,1,1,1,1,1,1,1]\n");
        for (previous, next) in [('a', 'b'), ('b', 'c'), ('c', 'd'), ('d', 'e'), ('e', 'f')] {
            yaml.push_str(&format!(
                "{next}: &{next} [{}]\n",
                vec![format!("*{previous}"); 10].join(",")
            ));
        }
        assert!(parse("aliases.yaml", yaml.as_bytes())
            .unwrap_err()
            .contains("limit"));
        assert!(parse(
            "duplicate.json",
            br#"{"openapi":"3.1.0","paths":{},"paths":{}}"#
        )
        .is_err());
    }

    #[test]
    fn scalar_alias_values_and_keys_cannot_amplify_past_byte_budget() {
        let anchor = "x".repeat(MAX_SCALAR_BYTES / 16);
        for entry in ["  - *large\n", "  - {*large : 0}\n"] {
            let yaml = format!(
                "openapi: 3.1.0\npaths: {{}}\nanchor: &large {anchor}\ncopies:\n{}",
                entry.repeat(20)
            );
            assert!(yaml.len() < MAX_BYTES);
            let error = parse("scalar-aliases.yaml", yaml.as_bytes()).unwrap_err();
            assert!(error.contains("scalar/key byte limit"), "{error}");
        }
    }

    #[test]
    fn scalar_and_key_charges_are_cumulative_before_retention() {
        use serde::de::DeserializeSeed;
        let mut nodes = 100;
        let mut bytes = 10;
        let error = Bounded {
            depth: 0,
            remaining: &mut nodes,
            scalar_bytes: &mut bytes,
            key: false,
        }
        .deserialize(serde_yaml::Deserializer::from_str("long_key: abcdef"))
        .unwrap_err();
        assert!(error.to_string().contains("scalar/key byte limit"));
        assert_eq!(bytes, 2); // The eight-byte map key was charged before its value.
        let mut nodes = 100;
        let mut bytes = 7;
        let error = Bounded {
            depth: 0,
            remaining: &mut nodes,
            scalar_bytes: &mut bytes,
            key: false,
        }
        .deserialize(serde_yaml::Deserializer::from_str("long_key: x"))
        .unwrap_err();
        assert!(error.to_string().contains("scalar/key byte limit"));
        assert_eq!(bytes, 7); // An oversized key fails before being retained.
        let mut nodes = 100;
        let mut bytes = 3;
        let error = Bounded {
            depth: 0,
            remaining: &mut nodes,
            scalar_bytes: &mut bytes,
            key: false,
        }
        .deserialize(serde::de::value::StringDeserializer::<serde_json::Error>::new("owned".into()))
        .unwrap_err();
        assert!(error.to_string().contains("scalar/key byte limit"));
    }

    #[test]
    fn fragments_include_exact_and_prefixed_candidates_until_full_path_is_known() {
        let doc = parse("contract.json",br#"{"openapi":"3.1.0","paths":{"/items":{"get":{}},"/api/items":{"get":{}},"/other/items":{"post":{}}}}"#).unwrap();
        let mut item = Item {
            id: "api".into(),
            kind: crate::catalog::Kind::Api,
            role: "http_handler".into(),
            label: "items".into(),
            symbol: "app.items".into(),
            path: "app.py".into(),
            line: 2,
            end_line: 4,
            fact_id: "handler".into(),
            evidence_ids: vec![],
            method: Some("GET".into()),
            route_path: Some("/items".into()),
            path_is_fragment: true,
        };
        let association = match_item(&doc, &item, true);
        assert_eq!(association["status"], "ambiguous");
        assert_eq!(association["candidates"].as_array().unwrap().len(), 2);
        assert!(association["basis"].as_str().unwrap().contains("fragment"));
        item.path_is_fragment = false;
        let association = match_item(&doc, &item, true);
        assert_eq!(association["status"], "candidate");
        assert_eq!(association["basis"], "exact method/path");
        assert_eq!(association["candidates"][0]["path"], "/items");
    }

    #[test]
    fn context_suffix_narrowing_mismatch_empty_fragment_and_budget_unknown() {
        let Some((e, _, mut item)) =
            crate::router_context::tests::extract(crate::router_context::tests::DIRECT)
        else {
            return;
        };
        let doc = parse("contract.json", br#"{"openapi":"3.1.0","paths":{"/api/caps/items":{"get":{}},"/tails/items":{"get":{}},"/caps":{"get":{}},"/unrelated":{"get":{"operationId":"handler"}}}}"#).unwrap();
        assert_eq!(match_item(&doc, &item, true)["status"], "ambiguous");
        let mut context = crate::router_context::derive(&e, &item);
        let matched = match_item_with_context(&doc, &item, true, &context);
        assert_eq!(matched["status"], "candidate");
        assert_eq!(matched["candidates"][0]["path"], "/api/caps/items");
        let ambiguous = parse("contract.json", br#"{"openapi":"3.1.0","paths":{"/api/caps/items":{"get":{}},"/other/caps/items":{"get":{}}}}"#).unwrap();
        assert_eq!(
            match_item_with_context(&ambiguous, &item, true, &context)["status"],
            "ambiguous"
        );
        context.candidates[0].composed_path = "/missing/items".into();
        assert_eq!(
            match_item_with_context(&doc, &item, true, &context)["status"],
            "context-mismatch"
        );
        item.route_path = Some("".into());
        assert_eq!(match_item(&doc, &item, false)["status"], "unmatched");
        context.candidates[0].composed_path = "/caps".into();
        assert_eq!(
            match_item_with_context(&doc, &item, false, &context)["candidates"][0]["path"],
            "/caps"
        );
        context.status = "budget-unknown".into();
        assert_eq!(
            match_item_with_context(&doc, &item, true, &context)["status"],
            "unknown"
        );
        context.status = "unknown".into();
        assert_eq!(
            match_item_with_context(&doc, &item, true, &context),
            match_item(&doc, &item, true)
        );
    }

    #[test]
    #[cfg(unix)]
    fn rejects_symlink_directory_and_oversized_file() {
        use std::io::Write;
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(b"openapi: 3.1.0\npaths: {}\n").unwrap();
        // macOS's temp directory may have a platform /var symlink ancestor.
        let real = f.path().canonicalize().unwrap();
        assert!(load(&real).is_ok());
        let dir = tempfile::tempdir().unwrap();
        let link = dir.path().canonicalize().unwrap().join("link");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        assert!(load(&link).is_err());
        assert!(load(&dir.path().canonicalize().unwrap()).is_err());
        f.as_file().set_len((MAX_BYTES + 1) as u64).unwrap();
        assert!(load(&real).is_err());
    }
}
