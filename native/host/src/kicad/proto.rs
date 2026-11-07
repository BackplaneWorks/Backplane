//! KiCad's API envelope and its messages: JSON <-> protobuf from the runtime's descriptor set, at run time (`prost-reflect`).
//! A Rust twin of the Bun helper's schema.ts and codec.ts: the agents' `kicad_ipc` JSON keeps its exact shape, which is not
//! proto3's canonical JSON but protobufjs's `fromObject`/`toObject`, so `prost-reflect`'s serde mapping cannot be used
//! (it would turn a FieldMask into a comma string and name fields in camelCase). Here, as there:
//!   - field names are the .proto's, a name not in the message is ignored;
//!   - in: scalars coerce as JavaScript does (`Number(x)`, `x|0`, `x>>>0`, `String(x)`, `Boolean(x)`), 64-bit integers take
//!     numbers, strings and `{low, high}`, an enum takes its name or a number and ignores any other word, `bytes` take base64;
//!     a wrong shape fails with protobufjs's words (`<.full.name>: object expected`, `array expected`), and after that
//!     `verify`'s `<oneof>: multiple values`;
//!   - out: only fields present, in field-number order, 64-bit integers as strings, enums by name (a number when unknown),
//!     bytes as base64, non-finite floats as strings;
//!   - a `google.protobuf.Any` is `{"@type": "<url>", ...fields}` when its type is known, `@type` last, else `{type_url, value}`.
//! Checked against protobufjs itself: test/files/fixtures/kipc_golden*.json are its output (test/files/kipc_golden.ts).
//! Known differences, all on garbage input: a 64-bit integer given a string that is not a decimal integer is refused here (Long
//! reads it as a wrong number and sends it); a type is found by its full name only (protobufjs also finds a short one, which
//! KiCad would then refuse); map fields come out sorted by key, not in wire order.
use prost::Message;
use prost_reflect::{prost_types as pt, DescriptorPool, DynamicMessage, FieldDescriptor, Kind, MapKey, MessageDescriptor, ReflectMessage, Value as Pv};
use serde_json::{json, Map, Number, Value};
use std::{collections::{HashMap, HashSet}, sync::{Arc, Mutex, OnceLock}};

pub const NO_SCHEMA: &str = "no API schema in this KiCad runtime";
pub const STATUS: [&str; 10] = ["AS_UNKNOWN", "AS_OK", "AS_TIMEOUT", "AS_BAD_REQUEST", "AS_NOT_READY", "AS_UNHANDLED", "AS_TOKEN_MISMATCH", "AS_BUSY", "AS_UNIMPLEMENTED", "AS_INTERNAL_ERROR"];
const PREFIX: &str = "type.googleapis.com/";
const ANY: &str = "google.protobuf.Any";
const LIMIT: usize = 100; // util.recursionLimit

// The envelope is fixed (common/envelope.proto), so it needs no schema.
#[derive(Clone, PartialEq, Message)] struct Any { #[prost(string, tag = "1")] type_url: String, #[prost(bytes = "vec", tag = "2")] value: Vec<u8> }
#[derive(Clone, PartialEq, Message)] struct ReqHeader { #[prost(string, tag = "1")] kicad_token: String, #[prost(string, tag = "2")] client_name: String }
#[derive(Clone, PartialEq, Message)] struct ApiRequest { #[prost(message, optional, tag = "1")] header: Option<ReqHeader>, #[prost(message, optional, tag = "2")] message: Option<Any> }
#[derive(Clone, PartialEq, Message)] struct ResHeader { #[prost(string, tag = "1")] kicad_token: String }
#[derive(Clone, PartialEq, Message)] struct ResStatus { #[prost(int32, tag = "1")] status: i32, #[prost(string, tag = "2")] error_message: String }
#[derive(Clone, PartialEq, Message)] struct ApiResponse { #[prost(message, optional, tag = "1")] header: Option<ResHeader>, #[prost(message, optional, tag = "2")] status: Option<ResStatus>, #[prost(message, optional, tag = "3")] message: Option<Any> }

// The schema ------------------------------------------------------------------------------------------------------------

/// The runtime's descriptor set when it ships one, else [`builtin`], which knows only Ping and GetVersion.
pub struct Schema { pool: DescriptorPool, full: bool }
impl Schema {
    pub fn message(&self, name: &str) -> Option<MessageDescriptor> { self.pool.get_message_by_name(name.strip_prefix('.').unwrap_or(name)) }
    /// Only Ping and GetVersion without a descriptor set.
    pub fn knows(&self, name: &str) -> bool { self.full || name == "kiapi.common.commands.Ping" || name == "kiapi.common.commands.GetVersion" }
}
fn message_of(name: &str, fields: &[(&str, i32, Option<(pt::field_descriptor_proto::Type, &str)>)]) -> pt::DescriptorProto {
    use pt::field_descriptor_proto::{Label, Type};
    pt::DescriptorProto { name: Some(name.into()), field: fields.iter().map(|(n, id, t)| pt::FieldDescriptorProto {
        name: Some((*n).into()), number: Some(*id), label: Some(Label::Optional as i32), r#type: Some(t.map(|x| x.0).unwrap_or(Type::Uint32) as i32), type_name: t.and_then(|x| (!x.1.is_empty()).then(|| x.1.to_string())), ..Default::default() }).collect(), ..Default::default() }
}
pub fn builtin() -> Arc<Schema> {
    static ONE: OnceLock<Arc<Schema>> = OnceLock::new();
    ONE.get_or_init(|| {
        use pt::field_descriptor_proto::Type;
        let file = |name: &str, package: &str, dep: &[&str], m: Vec<pt::DescriptorProto>| pt::FileDescriptorProto { name: Some(name.into()), package: Some(package.into()), syntax: Some("proto3".into()), dependency: dep.iter().map(|d| d.to_string()).collect(), message_type: m, ..Default::default() };
        let set = pt::FileDescriptorSet { file: vec![
            file("kiapi_types.proto", "kiapi.common.types", &[], vec![message_of("KiCadVersion", &[("major", 1, None), ("minor", 2, None), ("patch", 3, None), ("full_version", 4, Some((Type::String, "")))])]),
            file("kiapi_commands.proto", "kiapi.common.commands", &["kiapi_types.proto"], vec![message_of("Ping", &[]), message_of("GetVersion", &[]), message_of("GetVersionResponse", &[("version", 1, Some((Type::Message, ".kiapi.common.types.KiCadVersion")))])]),
        ] };
        Arc::new(Schema { pool: DescriptorPool::from_file_descriptor_set(set).expect("the built-in schema is valid"), full: false })
    }).clone()
}
pub fn from_bytes(bytes: &[u8]) -> Result<Schema, String> { DescriptorPool::decode(bytes).map(|pool| Schema { pool, full: true }).map_err(|e| e.to_string()) }

fn stamp(file: &str) -> Option<String> {
    use crate::plat::fs::MetadataExt;
    let m = std::fs::metadata(file).ok()?; Some(format!("{}:{}.{}", m.size(), m.mtime(), m.mtime_nsec()))
}
/// The schema for a descriptor-set file ("" or unreadable: the built-in one), read again when the file changes.
pub fn load(file: &str) -> Arc<Schema> {
    static CACHE: OnceLock<Mutex<HashMap<String, (String, Arc<Schema>)>>> = OnceLock::new();
    if file.is_empty() { return builtin() }
    let Some(st) = stamp(file) else { return builtin() };
    let mut c = CACHE.get_or_init(Default::default).lock().unwrap();
    if let Some((s, schema)) = c.get(file) { if *s == st { return schema.clone() } }
    let schema = std::fs::read(file).ok().and_then(|b| from_bytes(&b).ok()).map(Arc::new).unwrap_or_else(builtin);
    c.insert(file.into(), (st, schema.clone())); schema
}

// The envelope ----------------------------------------------------------------------------------------------------------

/// A command's JSON as the bytes of its `ApiRequest` (encodeRequest).
pub fn encode_request(s: &Schema, ty: &str, message: &Value, token: &str, client: &str) -> Result<Vec<u8>, String> {
    let t = s.message(ty).ok_or_else(|| format!("unknown command {ty}"))?;
    let empty = json!({});
    let body = encode_message(&t, if message.is_null() { &empty } else { message })?;
    Ok(ApiRequest { header: Some(ReqHeader { kicad_token: token.into(), client_name: client.into() }), message: Some(Any { type_url: format!("{PREFIX}{ty}"), value: body }) }.encode_to_vec())
}
/// `T.encode(T.fromObject(x))` after `T.verify`: the message alone, no envelope.
pub fn encode_message(t: &MessageDescriptor, message: &Value) -> Result<Vec<u8>, String> {
    let b = from_object(t, message, 0)?;
    if let Some(e) = b.bad { return Err(e) }
    Ok(b.m.encode_to_vec())
}

/// What KiCad said.
#[derive(Debug, PartialEq)]
pub enum Reply { Ok { ty: String, message: Value, token: String }, Err { status: String, error: String, token: String } }
impl Reply { pub fn token(&self) -> &str { match self { Reply::Ok { token, .. } | Reply::Err { token, .. } => token } } }

/// An `ApiResponse`'s bytes (decodeResponse): anything but AS_OK is that status and KiCad's words; a message of a type the
/// schema lacks is `{"@type", "bytes": base64}` (Ping's `google.protobuf.Empty` is one: the set does not hold it).
pub fn decode_response(s: &Schema, bytes: &[u8]) -> Result<Reply, String> {
    let r = ApiResponse::decode(bytes).map_err(|e| e.to_string())?;
    let token = r.header.map(|h| h.kicad_token).unwrap_or_default();
    let (code, words) = r.status.map(|x| (x.status, x.error_message)).unwrap_or((0, String::new()));
    if code != 1 { return Ok(Reply::Err { status: usize::try_from(code).ok().and_then(|i| STATUS.get(i)).map(|s| s.to_string()).unwrap_or_else(|| format!("AS_{code}")), error: words, token }) }
    let (url, value) = r.message.map(|m| (m.type_url, m.value)).unwrap_or_default();
    if url.is_empty() { return Ok(Reply::Ok { ty: String::new(), message: json!({}), token }) }
    let name = url.rsplit('/').next().unwrap_or("").to_string();
    let Some(t) = s.message(&name) else { return Ok(Reply::Ok { message: json!({"@type": name, "bytes": base64(&value)}), ty: name, token }) };
    let message = decode_message(&t, &value)?;
    Ok(Reply::Ok { ty: name, message, token })
}
/// `T.toObject(T.decode(bytes), TO)`.
pub fn decode_message(t: &MessageDescriptor, bytes: &[u8]) -> Result<Value, String> {
    let m = DynamicMessage::decode(t.clone(), bytes).map_err(|e| e.to_string())?;
    to_object(&m, 0)
}

// JSON in: protobufjs's fromObject and verify ----------------------------------------------------------------------------

struct Built { m: DynamicMessage, bad: Option<String> } // bad: what verify would say; a failed fromObject (Err) comes first
fn is_object(v: &Value) -> bool { v.is_object() || v.is_array() } // util.isObject

fn from_object(t: &MessageDescriptor, d: &Value, q: usize) -> Result<Built, String> {
    if t.full_name() == ANY { return any_from(t, d, q) }
    plain_from(t, d, q)
}
fn plain_from(t: &MessageDescriptor, d: &Value, q: usize) -> Result<Built, String> {
    if !is_object(d) { return Err(format!(".{}: object expected", t.full_name())) }
    if q > LIMIT { return Err("max depth exceeded".into()) }
    let mut m = DynamicMessage::new(t.clone()); let mut bad = None; let mut oneofs = HashSet::new();
    for f in t.fields() {
        let v = d.get(f.name()).unwrap_or(&Value::Null);
        let got = if f.is_map() { map_from(&f, v, q)? } else if f.is_list() { list_from(&f, v, q)? } else { one_from(&f, &f.kind(), v, q)? };
        let Some((pv, b)) = got else { continue };
        if let Some(o) = f.containing_oneof() { if !oneofs.insert(o.name().to_string()) && bad.is_none() { bad = Some(format!("{}: multiple values", o.name())) } }
        if bad.is_none() { bad = b }
        m.set_field(&f, pv);
    }
    Ok(Built { m, bad })
}
/// Any wrapper's fromObject: an `@type` the schema knows packs the object as that type; verify never looks inside it.
fn any_from(t: &MessageDescriptor, d: &Value, q: usize) -> Result<Built, String> {
    match d.get("@type").filter(|a| truthy(a)) {
        None => plain_from(t, d, q),
        Some(Value::String(at)) => {
            let name = at.rsplit('/').next().unwrap_or("");
            let Some(inner) = t.parent_pool().get_message_by_name(name.strip_prefix('.').unwrap_or(name)) else { return plain_from(t, d, q) };
            let value = from_object(&inner, d, q + 1)?.m.encode_to_vec();
            let url = at.strip_prefix('.').unwrap_or(at);
            let url = if url.contains('/') { url.to_string() } else { format!("/{url}") }; // the type_url prefix is optional, its slash is not
            let mut m = DynamicMessage::new(t.clone());
            m.set_field_by_name("type_url", Pv::String(url)); m.set_field_by_name("value", Pv::Bytes(value.into()));
            Ok(Built { m, bad: None })
        }
        Some(_) => Err("object[\"@type\"].substring is not a function".into()),
    }
}
fn full_name(f: &FieldDescriptor) -> String { format!(".{}", f.full_name()) }
type Field = Result<Option<(Pv, Option<String>)>, String>;

fn list_from(f: &FieldDescriptor, v: &Value, q: usize) -> Field {
    if !truthy(v) { return Ok(None) }
    let Value::Array(xs) = v else { return Err(format!("{}: array expected", full_name(f))) };
    let (mut out, mut bad) = (Vec::with_capacity(xs.len()), None); let k = f.kind();
    for x in xs { if let Some((pv, b)) = elem_from(f, &k, x, q)? { out.push(pv); if bad.is_none() { bad = b } } }
    Ok(Some((Pv::List(out), bad)))
}
fn map_from(f: &FieldDescriptor, v: &Value, q: usize) -> Field {
    if !truthy(v) { return Ok(None) }
    if !is_object(v) { return Err(format!("{}: object expected", full_name(f))) }
    let Some(entry) = f.kind().as_message().cloned() else { return Ok(None) };
    let (kf, vf) = (entry.map_entry_key_field(), entry.map_entry_value_field());
    let (mut out, mut bad) = (HashMap::new(), None);
    for (key, x) in v.as_object().into_iter().flatten() {
        let k = map_key(&kf.kind(), key);
        if k.is_none() && bad.is_none() { bad = Some(key_words(f.name(), &kf.kind())) } // verify's complaint, not fromObject's
        if let Some((pv, b)) = elem_from(f, &vf.kind(), x, q)? { if let Some(k) = k { out.insert(k, pv); } if bad.is_none() { bad = b } }
    }
    Ok(Some((Pv::Map(out), bad)))
}
fn map_key(k: &Kind, s: &str) -> Option<MapKey> {
    let digits = s.strip_prefix('-').unwrap_or(s); // util.key32Re, key64Re: no sign but '-', no leading zeros
    let int = !digits.is_empty() && digits.bytes().all(|c| c.is_ascii_digit()) && (digits == "0" || !digits.starts_with('0'));
    Some(match k {
        Kind::String => MapKey::String(s.into()),
        Kind::Bool => match s { "true" | "1" => MapKey::Bool(true), "false" | "0" => MapKey::Bool(false), _ => return None },
        Kind::Int32 | Kind::Sint32 | Kind::Sfixed32 if int => MapKey::I32(s.parse().ok()?), Kind::Uint32 | Kind::Fixed32 if int => MapKey::U32(s.parse().ok()?),
        Kind::Int64 | Kind::Sint64 | Kind::Sfixed64 if int => MapKey::I64(s.parse().ok()?), Kind::Uint64 | Kind::Fixed64 if int => MapKey::U64(s.parse().ok()?),
        _ => return None,
    })
}
fn key_words(name: &str, k: &Kind) -> String { // invalid(field, ...)
    let (what, ty) = match k {
        Kind::Bool => ("boolean key", "bool"), Kind::Int32 => ("integer key", "int32"), Kind::Sint32 => ("integer key", "sint32"), Kind::Sfixed32 => ("integer key", "sfixed32"), Kind::Uint32 => ("integer key", "uint32"), Kind::Fixed32 => ("integer key", "fixed32"),
        Kind::Int64 => ("integer|Long key", "int64"), Kind::Sint64 => ("integer|Long key", "sint64"), Kind::Sfixed64 => ("integer|Long key", "sfixed64"), Kind::Uint64 => ("integer|Long key", "uint64"), _ => ("integer|Long key", "fixed64"),
    };
    format!("{name}: {what}{{k:{ty}}} expected")
}
/// A singular field: null is no value (an enum simply matches nothing).
fn one_from(f: &FieldDescriptor, k: &Kind, v: &Value, q: usize) -> Field {
    let long_kind = matches!(k, Kind::Int64 | Kind::Sint64 | Kind::Sfixed64 | Kind::Uint64 | Kind::Fixed64);
    if v.is_null() || (long_kind && !f.supports_presence() && !v.is_object() && number(v) == 0.0) { return Ok(None) } // implicit presence: a zero is never read, so "" is no error
    elem_from(f, k, v, q)
}
/// genValuePartial_fromObject: one value, inside a list or a map or alone.
fn elem_from(f: &FieldDescriptor, k: &Kind, v: &Value, q: usize) -> Field {
    let plain = |p: Pv| -> Field { Ok(Some((p, None))) };
    match k {
        Kind::Message(t) => {
            if !is_object(v) { return Err(format!("{}: object expected", full_name(f))) }
            let b = from_object(t, v, q + 1)?; Ok(Some((Pv::Message(b.m), b.bad.map(|e| format!("{}.{e}", f.name())))))
        }
        Kind::Enum(e) => match v {
            Value::String(s) => Ok(e.get_value_by_name(s).map(|x| (Pv::EnumNumber(x.number()), None))),
            Value::Number(_) => { let n = number(v); Ok(((n as i32) as f64 == n).then_some((Pv::EnumNumber(n as i32), None))) }
            _ => Ok(None),
        },
        Kind::Double => plain(Pv::F64(number(v))),
        Kind::Float => plain(Pv::F32(number(v) as f32)),
        Kind::Int32 | Kind::Sint32 | Kind::Sfixed32 => plain(Pv::I32(to_int32(number(v)) as i32)),
        Kind::Uint32 | Kind::Fixed32 => plain(Pv::U32(to_int32(number(v)))),
        Kind::Int64 | Kind::Sint64 | Kind::Sfixed64 => plain(Pv::I64(long(v, false)? as i64)),
        Kind::Uint64 | Kind::Fixed64 => plain(Pv::U64(long(v, true)?)),
        Kind::Bool => plain(Pv::Bool(truthy(v))),
        Kind::String => plain(Pv::String(js_string(v))),
        Kind::Bytes => match v {
            Value::String(s) => plain(Pv::Bytes(unbase64(s)?.into())),
            Value::Array(xs) => plain(Pv::Bytes(xs.iter().map(|x| to_int32(number(x)) as u8).collect::<Vec<u8>>().into())),
            Value::Null => Err("Cannot read properties of null (reading 'length')".into()),
            _ => Ok(None),
        },
    }
}

fn truthy(v: &Value) -> bool { // Boolean(x)
    match v { Value::Null => false, Value::Bool(b) => *b, Value::Number(_) => { let n = number(v); n != 0.0 && !n.is_nan() } Value::String(s) => !s.is_empty(), _ => true }
}
/// Number(x).
fn number(v: &Value) -> f64 {
    match v {
        Value::Null => 0.0, Value::Bool(b) => *b as u8 as f64, Value::Number(n) => n.as_f64().unwrap_or(f64::NAN), Value::String(s) => str_number(s),
        Value::Array(a) => match a.as_slice() { [] => 0.0, [x] => if x.is_null() { 0.0 } else { str_number(&js_string(x)) }, _ => f64::NAN },
        Value::Object(_) => f64::NAN,
    }
}
fn str_number(s: &str) -> f64 {
    let t = s.trim();
    if t.is_empty() { return 0.0 }
    let radix = |p: &str, r: u32| t.strip_prefix(p).or(t.strip_prefix(&p.to_uppercase())).map(|d| u64::from_str_radix(d, r).map(|n| n as f64).unwrap_or(f64::NAN));
    if let Some(n) = radix("0x", 16).or(radix("0o", 8)).or(radix("0b", 2)) { return n }
    match t { "Infinity" | "+Infinity" => return f64::INFINITY, "-Infinity" => return f64::NEG_INFINITY, _ => {} }
    if t.chars().all(|c| c.is_ascii_digit() || matches!(c, '+' | '-' | '.' | 'e' | 'E')) { t.parse().unwrap_or(f64::NAN) } else { f64::NAN } // Rust's parse also takes "inf" and "nan"
}
/// ToInt32/ToUint32's bits (`x|0`, `x>>>0`).
fn to_int32(n: f64) -> u32 { if !n.is_finite() { 0 } else { n.trunc().rem_euclid(4294967296.0) as u32 } }
/// Long.fromValue(x, unsigned), as the 64 bits.
fn long(v: &Value, unsigned: bool) -> Result<u64, String> {
    let of = |n: f64| -> u64 { if n.is_nan() { 0 } else if unsigned { if n <= 0.0 { 0 } else { n as u64 } } else { n as i64 as u64 } };
    match v {
        Value::Number(n) => Ok(n.as_i64().map(|x| if unsigned && x < 0 { 0 } else { x as u64 }).or(n.as_u64()).unwrap_or_else(|| of(number(v)))),
        Value::String(s) => {
            match s.as_str() { "" => return Err("empty string".into()), "NaN" | "Infinity" | "+Infinity" | "-Infinity" => return Ok(0), _ => {} }
            let (neg, digits) = s.strip_prefix('-').map(|d| (true, d)).unwrap_or((false, s));
            if digits.is_empty() || !digits.bytes().all(|c| c.is_ascii_digit()) { return Err(format!("{s}: not an integer")) }
            let n = digits.bytes().fold(0u64, |a, c| a.wrapping_mul(10).wrapping_add((c - b'0') as u64)); // Long wraps too
            Ok(if neg { n.wrapping_neg() } else { n })
        }
        Value::Object(o) => { let w = |k: &str| o.get(k).map(number).map(to_int32).unwrap_or(0) as u64; Ok(w("high") << 32 | w("low")) }
        Value::Null => Err("Cannot read properties of null (reading 'low')".into()),
        _ => Ok(0),
    }
}
/// String(x).
fn js_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(), Value::Null => "null".into(), Value::Bool(b) => b.to_string(),
        Value::Number(n) => match n.as_i64() { Some(i) => i.to_string(), None => { let x = number(v); if x.fract() == 0.0 && x.abs() < 1e21 { format!("{x:.0}") } else { x.to_string() } } },
        Value::Array(a) => a.iter().map(|x| if x.is_null() { String::new() } else { js_string(x) }).collect::<Vec<_>>().join(","),
        Value::Object(_) => "[object Object]".into(),
    }
}

// JSON out: protobufjs's toObject {longs: String, enums: String, bytes: String, defaults: false, json: true} -------------

fn to_object(m: &DynamicMessage, q: usize) -> Result<Value, String> {
    if q > LIMIT { return Err("max depth exceeded".into()) }
    if m.descriptor().full_name() == ANY { if let Some(o) = any_to(m, q)? { return Ok(o) } }
    let mut fields: Vec<FieldDescriptor> = m.descriptor().fields().collect(); fields.sort_by_key(|f| f.number());
    let mut d = Map::new();
    for f in fields {
        if !m.has_field(&f) { continue }
        let v = m.get_field(&f); let k = f.kind();
        let out = match &*v {
            Pv::List(xs) => Value::Array(xs.iter().map(|x| value_to(&k, x, q)).collect::<Result<_, _>>()?),
            Pv::Map(xs) => {
                let vk = k.as_message().map(|e| e.map_entry_value_field().kind()).ok_or("map without an entry")?;
                let mut ks: Vec<(String, &Pv)> = xs.iter().map(|(a, b)| (key_text(a), b)).collect(); ks.sort_by(|a, b| a.0.cmp(&b.0));
                let mut o = Map::new(); for (a, b) in ks { o.insert(a, value_to(&vk, b, q)?); } Value::Object(o)
            }
            x => value_to(&k, x, q)?,
        };
        d.insert(f.name().to_string(), out);
    }
    Ok(Value::Object(d))
}
fn key_text(k: &MapKey) -> String { match k { MapKey::Bool(b) => b.to_string(), MapKey::I32(n) => n.to_string(), MapKey::I64(n) => n.to_string(), MapKey::U32(n) => n.to_string(), MapKey::U64(n) => n.to_string(), MapKey::String(s) => s.clone() } }
/// A known type's fields and its `@type`, last; None to print the Any as it stands.
fn any_to(m: &DynamicMessage, q: usize) -> Result<Option<Value>, String> {
    let url = m.get_field_by_name("type_url").and_then(|v| v.as_str().map(String::from)).unwrap_or_default();
    if url.is_empty() { return Ok(None) }
    let (prefix, name) = url.rsplit_once('/').map(|(a, b)| (format!("{a}/"), b.to_string())).unwrap_or((String::new(), url.clone()));
    let Some(t) = m.descriptor().parent_pool().get_message_by_name(name.strip_prefix('.').unwrap_or(&name)) else { return Ok(None) };
    let value = m.get_field_by_name("value").and_then(|v| v.as_bytes().cloned()).unwrap_or_default();
    let inner = DynamicMessage::decode(t.clone(), value).map_err(|e| e.to_string())?;
    let mut o = to_object(&inner, q + 1)?;
    if let Value::Object(d) = &mut o { d.insert("@type".into(), Value::String(format!("{}{}", if prefix.is_empty() { PREFIX } else { &prefix }, t.full_name()))); }
    Ok(Some(o))
}
fn value_to(k: &Kind, v: &Pv, q: usize) -> Result<Value, String> {
    Ok(match v {
        Pv::Bool(b) => Value::Bool(*b), Pv::I32(n) => json!(*n), Pv::U32(n) => json!(*n),
        Pv::I64(n) => Value::String(n.to_string()), Pv::U64(n) => Value::String(n.to_string()),
        Pv::F32(x) => float(*x as f64), Pv::F64(x) => float(*x),
        Pv::String(s) => Value::String(s.clone()), Pv::Bytes(b) => Value::String(base64(b)),
        Pv::EnumNumber(n) => match k.as_enum().and_then(|e| e.get_value(*n)) { Some(x) => Value::String(x.name().to_string()), None => json!(*n) },
        Pv::Message(m) => to_object(m, q + 1)?,
        Pv::List(_) | Pv::Map(_) => return Err("a list inside a list".into()),
    })
}
/// A float as JavaScript prints it: 1, not 1.0; the non-finite as strings.
fn float(x: f64) -> Value {
    if !x.is_finite() { return Value::String(if x.is_nan() { "NaN" } else if x > 0.0 { "Infinity" } else { "-Infinity" }.into()) }
    if x.fract() == 0.0 && x.abs() < 9007199254740992.0 { return Value::Number(Number::from(x as i64)) }
    Number::from_f64(x).map(Value::Number).unwrap_or(Value::Null)
}

// base64, as @protobufjs/base64 ------------------------------------------------------------------------------------------

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
pub fn base64(b: &[u8]) -> String {
    let mut o = String::with_capacity(b.len().div_ceil(3) * 4);
    for c in b.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..4 { if i <= c.len() { o.push(B64[(n >> (18 - 6 * i) & 63) as usize] as char) } else { o.push('=') } }
    }
    o
}
/// '-' and '_' read as '+' and '/', a '=' after two digits ends it, a dangling digit or anything else is "invalid encoding".
pub fn unbase64(s: &str) -> Result<Vec<u8>, String> {
    let (mut o, mut j, mut t) = (Vec::with_capacity(s.len() / 4 * 3 + 3), 0, 0u8);
    for c in s.bytes() {
        if c == b'=' && j > 1 { break }
        let d = match c { b'-' => 62, b'_' => 63, _ => B64.iter().position(|x| *x == c).ok_or("invalid encoding")? as u8 };
        match j { 0 => {} 1 => o.push(t << 2 | (d & 48) >> 4), 2 => o.push((t & 15) << 4 | (d & 60) >> 2), _ => o.push((t & 3) << 6 | d) }
        t = d; j = (j + 1) % 4;
    }
    if j == 1 { return Err("invalid encoding".into()) }
    Ok(o)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `name` under the repository's test/, from whichever crate compiled this (the hub, or test/files/kipc-probe).
    fn fixture(name: &str) -> String {
        let at = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().find(|d| d.join("test/fixtures/kipc").is_dir()).expect("the repository's test folder");
        at.join("test").join(name).to_string_lossy().into()
    }
    /// numbers compare by value, whatever their text ("1e-7" is 1e-07; a golden is JavaScript's, the hub's serde_json may keep raw text)
    fn same(a: &Value, b: &Value) -> bool {
        match (a, b) {
            (Value::Number(_), Value::Number(_)) => a.as_f64() == b.as_f64(),
            (Value::Array(x), Value::Array(y)) => x.len() == y.len() && x.iter().zip(y).all(|(p, q)| same(p, q)),
            (Value::Object(x), Value::Object(y)) => x.len() == y.len() && x.iter().zip(y.iter()).all(|((k, p), (l, q))| k == l && same(p, q)), // in order: field order is part of the shape
            _ => a == b,
        }
    }
    fn hex_of(s: &str) -> Vec<u8> { (0..s.len() / 2).map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap()).collect() }
    fn to_hex(b: &[u8]) -> String { b.iter().map(|x| format!("{x:02x}")).collect() }

    /// Each case is protobufjs's own answer: `in` (JSON) -> `hex` or `error`, and the reverse, `hex` -> `out`.
    fn golden(s: &Schema, file: &str) -> usize {
        let cases: Value = serde_json::from_str(&std::fs::read_to_string(fixture(file)).unwrap()).unwrap();
        let mut n = 0;
        for c in cases.as_array().unwrap() {
            let ty = c["type"].as_str().unwrap(); let t = s.message(ty).unwrap_or_else(|| panic!("no {ty}"));
            if let Some(i) = c.get("in") {
                match (encode_message(&t, i), c.get("hex").and_then(Value::as_str), c.get("error").and_then(Value::as_str)) {
                    (Ok(b), Some(h), _) => assert_eq!(to_hex(&b), h, "{ty} {i}"),
                    (Err(e), _, Some(w)) => assert_eq!(e, w, "{ty} {i}"),
                    (r, h, e) => panic!("{ty} {i}: {r:?}, wanted {h:?} {e:?}"),
                }
            }
            if let (Some(h), Some(o)) = (c.get("hex").and_then(Value::as_str), c.get("out")) {
                let got = decode_message(&t, &hex_of(h)).unwrap(); assert!(same(&got, o), "{ty} {h}\n got {got}\nwant {o}");
            }
            n += 1;
        }
        n
    }
    #[test] fn golden_fixture_schema() {
        let s = from_bytes(&std::fs::read(fixture("fixtures/kipc/api.desc")).unwrap()).unwrap();
        assert!(golden(&s, "files/fixtures/kipc_golden_fixture.json") >= 10);
    }
    /// The runtime's descriptor set is KiCad's and stays out of this repository: on a machine without the runtime this says so and passes.
    #[test] fn golden_runtime_schema() {
        let p = std::env::var("BACKPLANE_TEST_API_DESC").unwrap_or_else(|_| format!("{}/.backplane-bend/kicad/10.99.0-bp.4/share/backplane/api.desc", std::env::var("HOME").unwrap_or_default()));
        let Ok(b) = std::fs::read(&p) else { eprintln!("SKIPPED golden_runtime_schema: no {p}"); return };
        let s = from_bytes(&b).unwrap();
        assert!(golden(&s, "files/fixtures/kipc_golden_runtime.json") >= 20);
    }
    fn reply_of(status: i32, words: &str, any: Option<(&str, Vec<u8>)>) -> Vec<u8> {
        ApiResponse { header: Some(ResHeader { kicad_token: "tok".into() }), status: Some(ResStatus { status, error_message: words.into() }), message: any.map(|(u, value)| Any { type_url: u.into(), value }) }.encode_to_vec()
    }
    #[test] fn request_envelope() {
        let s = builtin();
        let b = encode_request(&s, "kiapi.common.commands.Ping", &Value::Null, "", "backplane-1").unwrap();
        // the bytes the wire probe sent the real server, but for the client's name
        let url = b"type.googleapis.com/kiapi.common.commands.Ping";
        assert_eq!(to_hex(&b), format!("0a0d120b{}12{:02x}0a{:02x}{}", to_hex(b"backplane-1"), 2 + url.len(), url.len(), to_hex(url)));
        let r = ApiRequest::decode(&*encode_request(&s, "kiapi.common.commands.GetVersion", &json!({}), "t", "c").unwrap()).unwrap();
        assert_eq!(r.header.unwrap(), ReqHeader { kicad_token: "t".into(), client_name: "c".into() });
        assert_eq!(r.message.unwrap(), Any { type_url: "type.googleapis.com/kiapi.common.commands.GetVersion".into(), value: vec![] });
        assert_eq!(encode_request(&s, "kiapi.x.Nope", &json!({}), "", "c").unwrap_err(), "unknown command kiapi.x.Nope");
    }
    #[test] fn response_envelope() {
        let s = builtin();
        // the bytes of the real server's answer to GetVersion, read off the wire (bp.4)
        let real = "0a260a2461383633386330322d313231342d346639382d623232352d623236663764353337366539120208011a5c0a3c747970652e676f6f676c65617069732e636f6d2f6b696170692e636f6d6d6f6e2e636f6d6d616e64732e47657456657273696f6e526573706f6e7365121c0a1a080a1063221431302e39392e302d756e6b6e6f776e2d62702e34";
        let r = decode_response(&s, &hex_of(real)).unwrap();
        assert_eq!(r, Reply::Ok { ty: "kiapi.common.commands.GetVersionResponse".into(), token: "a8638c02-1214-4f98-b225-b26f7d5376e9".into(), message: json!({"version":{"major":10,"minor":99,"full_version":"10.99.0-unknown-bp.4"}}) });
        // and to Ping: Empty is in no schema, as the helper shows it
        let real = "0a260a2461383633386330322d313231342d346639382d623232352d623236663764353337366539120208011a2b0a29747970652e676f6f676c65617069732e636f6d2f676f6f676c652e70726f746f6275662e456d707479";
        assert_eq!(decode_response(&s, &hex_of(real)).unwrap(), Reply::Ok { ty: "google.protobuf.Empty".into(), token: "a8638c02-1214-4f98-b225-b26f7d5376e9".into(), message: json!({"@type":"google.protobuf.Empty","bytes":""}) });
        assert_eq!(decode_response(&s, &reply_of(3, "no", None)).unwrap(), Reply::Err { status: "AS_BAD_REQUEST".into(), error: "no".into(), token: "tok".into() });
        assert_eq!(decode_response(&s, &reply_of(42, "", None)).unwrap(), Reply::Err { status: "AS_42".into(), error: "".into(), token: "tok".into() });
        assert_eq!(decode_response(&s, &reply_of(1, "", None)).unwrap(), Reply::Ok { ty: "".into(), message: json!({}), token: "tok".into() });
        assert_eq!(decode_response(&s, &reply_of(1, "", Some(("type.googleapis.com/x.Y", vec![1, 2, 3])))).unwrap(), Reply::Ok { ty: "x.Y".into(), message: json!({"@type":"x.Y","bytes":"AQID"}), token: "tok".into() });
        assert!(decode_response(&s, &[0xff]).is_err());
    }
    #[test] fn builtin_knows_two() {
        let s = builtin();
        assert!(s.knows("kiapi.common.commands.Ping") && s.knows("kiapi.common.commands.GetVersion") && !s.knows("kiapi.board.commands.GetItems"));
        assert!(load("").pool.get_message_by_name("kiapi.common.types.KiCadVersion").is_some());
        assert!(!load("/nonexistent/api.desc").full);
        assert!(load(&fixture("fixtures/kipc/api.desc")).full);
    }
    #[test] fn base64_round_trip() {
        let cases: [&[u8]; 6] = [b"", b"f", b"fo", b"foo", b"foob", &[0xfb, 0xff, 0xfe]];
        for b in cases { assert_eq!(unbase64(&base64(b)).unwrap(), b) }
        assert_eq!(base64(b"foob"), "Zm9vYg=="); assert_eq!(unbase64("-_-_").unwrap(), unbase64("+/+/").unwrap()); assert_eq!(unbase64("Zg").unwrap(), b"f");
        assert_eq!(unbase64("Z").unwrap_err(), "invalid encoding"); assert_eq!(unbase64("Z!").unwrap_err(), "invalid encoding");
    }
    #[test] fn js_coercions() {
        for (v, n) in [(json!("12"), 12.0), (json!(" 1e3 "), 1000.0), (json!(""), 0.0), (json!(true), 1.0), (json!([]), 0.0), (json!([7]), 7.0), (json!("0x10"), 16.0), (json!(1.5), 1.5)] { assert_eq!(number(&v), n, "{v}") }
        for v in [json!("abc"), json!({}), json!([1, 2]), json!("inf"), json!("nan")] { assert!(number(&v).is_nan(), "{v}") }
        assert_eq!(to_int32(-1.0), u32::MAX); assert_eq!(to_int32(4294967297.0), 1); assert_eq!(to_int32(f64::NAN), 0); assert_eq!(to_int32(1.9), 1);
        assert_eq!(long(&json!("-5"), false), Ok((-5i64) as u64)); assert_eq!(long(&json!("18446744073709551615"), true), Ok(u64::MAX)); assert_eq!(long(&json!(-5), true), Ok(0));
        assert_eq!(long(&json!({"low":1,"high":1}), true), Ok((1 << 32) + 1)); assert!(long(&json!("12x"), false).is_err()); assert!(long(&json!(""), false).is_err());
        assert_eq!(js_string(&json!(5)), "5"); assert_eq!(js_string(&json!(true)), "true"); assert_eq!(js_string(&json!({})), "[object Object]"); assert_eq!(js_string(&json!([1, "a"])), "1,a");
        assert_eq!(float(1.0), json!(1)); assert_eq!(float(f64::INFINITY), json!("Infinity")); assert_eq!(float(f64::NAN), json!("NaN"));
    }
}
