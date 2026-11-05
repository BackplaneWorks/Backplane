//! Actor-private native sidebar navigation (core/native-sidebar.bend Ns.*).
//! Only the session actor supplies `local` and the hub supplies `nativeKey`.
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

const PREFIX: &str = "local.native.sidebar.";
pub fn text<'a>(v: &'a Value, k: &str) -> &'a str { v[k].as_str().unwrap_or("") }
fn parse(s: &str) -> Value { serde_json::from_str(s).unwrap_or(Value::Null) }
fn hash(s: &str) -> String { hex::encode(Sha256::digest(s.as_bytes())) }
fn show(v: &Value) -> String { serde_json::to_string(v).unwrap() }
fn same(a: &Value, b: &Value) -> bool {
    ["hub", "generation", "epoch", "viewId"].iter().all(|k| text(a, k) == text(b, k))
        && a["target"]["kind"] == b["target"]["kind"] && a["target"]["id"] == b["target"]["id"]
}
fn key_valid(k: &str) -> bool { hex::decode(k).map_or(false, |b| b.len() == 16) }
fn scope(kind: &str, owner: &str) -> String { hash(&show(&json!({"kind":kind,"owner":owner}))) }
fn owner_digest(owner: &str) -> String { hash(&format!("backplane.native-sidebar.owner.v1\n{owner}")) }
fn choices(mode: &str, query: &str, open: &Value) -> String {
    // Bend Map.to_list is in key order; only Bool flags belong to Session.flags.
    let sorted: std::collections::BTreeMap<&String, bool> = open.as_object().into_iter().flatten()
        .filter_map(|(k,v)| v.as_bool().map(|v| (k,v))).collect();
    hash(&show(&json!({"mode":mode,"query":query,"open":sorted})))
}
fn mac(key: &str, payload: &Value) -> String {
    let Ok(k) = hex::decode(key) else { return String::new() };
    let mut inner = [0x36; 64]; let mut outer = [0x5c; 64];
    for (i,b) in k.iter().enumerate().take(64) { inner[i] ^= b; outer[i] ^= b; }
    let mut h = Sha256::new(); h.update(inner); h.update(b"backplane.native-sidebar.v1\n"); h.update(show(payload).as_bytes());
    let mut out = Sha256::new(); out.update(outer); out.update(h.finalize()); hex::encode(out.finalize())
}
fn ct_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() { return false }
    a.bytes().zip(b.bytes()).fold(0u8, |d,(a,b)| d | (a^b)) == 0
}
fn stored(p: &Value) -> Value {
    json!({"offset":p["offset"],"limit":p["limit"],"metadataRevision":p["metadataRevision"],
        "choicesDigest":p["choicesDigest"],"membershipDigest":p["membershipDigest"]})
}

pub struct NativeSidebar {
    ctx: Value, key: String, pages: Value, budget: Value, owners: Vec<String>,
    /// Reconciled from the actual lists rendered, never from unrendered cached positions.
    next_pages: Map<String, Value>,
}
impl NativeSidebar {
    /// Ns.offered/prepare: identity changes discard the budget; revision or choices discard positions.
    pub fn prepare(p: &Value, mode: &str, query: &str, open: &Value, owners: Vec<String>) -> Option<Self> {
        if p["nativeSidebar"] != json!(1) || !key_valid(text(p,"nativeKey")) { return None }
        let old_ctx = parse(text(&p["local"], &format!("{PREFIX}context")));
        let mut ctx = json!({"hub":text(p,"hub"),"generation":text(p,"generation"),"epoch":text(p,"epoch"),
            "viewId":text(p,"viewId"),"target":p["target"],"metadataRevision":p["revision"].as_u64().unwrap_or(0)});
        // Obj.put prepends the choices field in Bend. It is not part of context_digest.
        let digest = choices(mode, query, open);
        let mut with_choices = json!({"choicesDigest":digest});
        with_choices.as_object_mut().unwrap().extend(ctx.as_object().unwrap().clone());
        ctx = with_choices;
        let identity = same(&old_ctx, &ctx);
        let pages = if identity && old_ctx["metadataRevision"] == ctx["metadataRevision"] && old_ctx["choicesDigest"] == ctx["choicesDigest"] {
            parse(text(&p["local"], &format!("{PREFIX}pages")))
        } else { json!({}) };
        let budget = if identity { parse(text(&p["local"], &format!("{PREFIX}budget"))) } else { Value::Null };
        Some(Self { ctx, key:text(p,"nativeKey").into(), pages, budget, owners, next_pages:Map::new() })
    }
    pub fn project_limit(&self) -> usize { self.budget["projects"].as_u64().unwrap_or(8).clamp(1,8) as usize }
    pub fn row_budget(&self) -> usize { self.budget["rows"].as_u64().unwrap_or(48).clamp(1,48) as usize }
    fn context_digest(&self) -> String {
        hash(&format!("backplane.native-sidebar.context.v1\n{}", show(&json!({"hub":self.ctx["hub"],"generation":self.ctx["generation"],
            "epoch":self.ctx["epoch"],"viewId":self.ctx["viewId"],"target":self.ctx["target"]}))))
    }
    fn payload(&self, kind: &str, owner: &str, member: &str, offset: usize, limit: usize) -> Value {
        json!({"version":1,"hub":self.ctx["hub"],"generation":self.ctx["generation"],"epoch":self.ctx["epoch"],
            "viewId":self.ctx["viewId"],"target":self.ctx["target"],"metadataRevision":self.ctx["metadataRevision"],
            "listKind":kind,"projectOwner":owner,"choicesDigest":self.ctx["choicesDigest"],"membershipDigest":member,"offset":offset,"limit":limit})
    }
    fn token(&self, payload: &Value) -> String {
        show(&json!({"payload":{"version":payload["version"],"contextDigest":self.context_digest(),"metadataRevision":payload["metadataRevision"],
            "listKind":payload["listKind"],"ownerDigest":owner_digest(text(payload,"projectOwner")),"choicesDigest":payload["choicesDigest"],
            "membershipDigest":payload["membershipDigest"],"offset":payload["offset"],"limit":payload["limit"]},"signature":mac(&self.key,payload)}))
    }
    fn expand(&self, c: &Value) -> Option<Value> {
        let d = text(c,"ownerDigest");
        let found: Vec<&str> = if d == owner_digest("") { vec![""] } else { self.owners.iter().filter(|o| owner_digest(o) == d).map(String::as_str).collect() };
        if found.len()!=1 { return None }
        let mut p = self.payload(text(c,"listKind"),found[0],text(c,"membershipDigest"),0,1);
        for k in ["version","metadataRevision","choicesDigest","offset","limit"] { p[k] = c[k].clone(); }
        Some(p)
    }
    /// Ns.valid: the paired hub prefix and exact canonical MAC bind a cursor to this actor's private context.
    fn valid(&self, value: &str) -> Option<Value> {
        let raw = value.strip_prefix(&format!("{}|",text(&self.ctx,"hub")))?;
        if raw.is_empty() || raw.len()>2048 { return None }
        let token = parse(raw); let c = &token["payload"];
        if c["version"] != json!(1) || text(c,"contextDigest") != self.context_digest()
            || c["offset"].as_u64().filter(|n| *n <= 281_474_976_710_655).is_none()
            || c["limit"].as_u64().filter(|n| (1..=48).contains(n)).is_none() { return None }
        let p = self.expand(c)?;
        ct_eq(text(&token,"signature"),&mac(&self.key,&p)).then_some(p)
    }
    /// Ns.accept: stale signed metadata/choices clears positions; forged or foreign cursors change nothing.
    pub fn accept(&mut self, value: &str) -> bool {
        let Some(p) = self.valid(value) else { return false };
        if p["metadataRevision"] != self.ctx["metadataRevision"] || p["choicesDigest"] != self.ctx["choicesDigest"] {
            self.pages = json!({}); return true
        }
        if !self.pages.is_object() || self.pages.as_object().unwrap().len()>=64 { self.pages=json!({}); }
        self.pages[scope(text(&p,"listKind"),text(&p,"projectOwner"))] = stored(&p); true
    }
    fn control(&self, on: bool, kind: &str, owner: &str, member: &str, offset: usize, limit: usize, label: String) -> Value {
        if !on { return Value::Null }
        json!({"label":label,"action":"native-sidebar-page","value":format!("{}|{}",text(&self.ctx,"hub"),self.token(&self.payload(kind,owner,member,offset,limit)))})
    }
    /// Ns.slice/position + reconcile. Rows retain source order, and a selected exception costs no ordinary slot.
    pub fn slice(&mut self, rows: Vec<Value>, kind: &str, owner: &str, limit: usize, selected: &str) -> (Vec<Value>,Value) {
        let ids: Vec<&str> = rows.iter().map(|r|text(r,"id").split_once('|').map_or(text(r,"id"),|(_,i)|i)).collect();
        let (indices,pager)=self.slice_ids(&ids,kind,owner,limit,selected);
        (indices.into_iter().map(|i| rows[i].clone()).collect(),pager)
    }
    /// Page identities before rendering rows: the real-size state never needs a JSON body per thread.
    pub fn slice_ids(&mut self, ids: &[&str], kind: &str, owner: &str, limit: usize, selected: &str) -> (Vec<usize>,Value) {
        if ids.is_empty() { return (vec![],Value::Null) }
        let member=hash(&show(&json!(ids))); let total=ids.len(); let limit=limit.clamp(1,48);
        let saved=&self.pages[scope(kind,owner)];
        let offset=saved["offset"].as_u64().filter(|n| *n < total as u64 && saved["limit"]==json!(limit)
            && text(saved,"membershipDigest")==member && saved["metadataRevision"]==self.ctx["metadataRevision"] && saved["choicesDigest"]==self.ctx["choicesDigest"]).unwrap_or(0) as usize;
        let end=(offset+limit).min(total);
        let extra=!selected.is_empty() && ids.contains(&selected) && !ids[offset..end].contains(&selected);
        let pager=json!({"version":1,"total":total,"offset":offset,"limit":limit,"selectedOutside":extra,
            "previous":self.control(offset>0,kind,owner,&member,offset.saturating_sub(limit),limit,"Previous".into()),
            "next":self.control(end<total,kind,owner,&member,end,limit,format!("Next · {total} total"))});
        // Ns.reconcile only retains lists having a control (one-page lists need no saved position).
        if offset>0 || end<total { self.next_pages.insert(scope(kind,owner),stored(&self.payload(kind,owner,&member,offset,limit))); }
        let mut indices: Vec<usize>=(offset..end).collect();
        if extra { indices.push(ids.iter().position(|id|*id==selected).unwrap()); }
        (indices,pager)
    }
    pub fn local(self) -> Value {
        json!({"local.native.sidebar.pages":show(&Value::Object(self.next_pages)),"local.native.sidebar.context":show(&self.ctx),
            "local.native.sidebar.key":self.key,"local.native.sidebar.budget":if self.budget.is_null(){String::new()}else{show(&self.budget)}})
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request() -> Value { json!({"nativeSidebar":1,"hub":"h","generation":"1","epoch":"epoch","viewId":"view", "revision":7,
        "target":{"kind":"thread","id":"a"},"nativeKey":"00112233445566778899aabbccddeeff"}) }
    fn ns(p: &Value) -> NativeSidebar { NativeSidebar::prepare(p,"projects","",&json!({}),vec!["p".into()]).unwrap() }
    fn rows(n: usize) -> Vec<Value> { (1..=n).rev().map(|i|json!({"id":format!("h|{i}")})).collect() }
    fn next(pager: &Value) -> &str { text(&pager["next"],"value") }
    fn restored(p: &Value, local: Value) -> NativeSidebar { let mut p=p.clone(); p["local"]=local; ns(&p) }
    #[test] fn native_sidebar_external_mac_vector() {
        // Same external vector checked by test/native_sidebar_page_test.bend.
        let p=json!({"version":1,"hub":"h","generation":"1","epoch":"epoch","viewId":"view","target":{"kind":"thread","id":"a"},
            "metadataRevision":7,"listKind":"settled","projectOwner":"p","choicesDigest":"external-vector","membershipDigest":"ordered-ids","offset":16,"limit":16});
        assert_eq!(mac("00112233445566778899aabbccddeeff",&p),"31b6ca450712a692a077c3b9f85e0a6387a4d18195c3ef6c1fea86dedaf36a72");
    }
    #[test] fn native_sidebar_walk_is_contiguous_and_selected_exception_is_appended() {
        for n in [1,8,16,17,48,49,200] {
            let p=request(); let mut nav=ns(&p); let mut seen=vec![];
            loop {
                let (slice,pager)=nav.slice(rows(n),"settled","p",16,"2");
                let offset=pager["offset"].as_u64().unwrap() as usize; let ordinary=16.min(n-offset);
                seen.extend(slice.iter().take(ordinary).map(|r|text(r,"id").to_string()));
                assert!(slice.len()<=17); assert_eq!(pager["total"],n);
                if pager["selectedOutside"]==true { assert_eq!(text(slice.last().unwrap(),"id"),"h|2"); }
                let token=next(&pager).to_string(); if token.is_empty() { break }
                nav=restored(&p,nav.local()); assert!(nav.accept(&token));
            }
            assert_eq!(seen,rows(n).iter().map(|r|text(r,"id").to_string()).collect::<Vec<_>>());
        }
    }
    #[test] fn native_sidebar_seal_binds_all_payload_fields_and_actor_context() {
        let p=request(); let mut nav=ns(&p); let (_,pager)=nav.slice(rows(200),"settled","p",16,""); let control=next(&pager);
        assert!(nav.valid(control).is_some()); assert!(nav.valid(control.trim_start_matches("h|")).is_none());
        let token=parse(control.strip_prefix("h|").unwrap());
        for (k,v) in [("offset",json!(999)),("limit",json!(48)),("ownerDigest",json!(owner_digest("other"))),
            ("choicesDigest",json!("forged")),("membershipDigest",json!("forged")),("listKind",json!("threads")),("metadataRevision",json!(8))] {
            let mut t=token.clone(); t["payload"][k]=v; assert!(nav.valid(&format!("h|{}",show(&t))).is_none(),"{k}");
        }
        let local=nav.local();
        for (k,v) in [("hub","other"),("generation","2"),("viewId","other"),("epoch","other"),("nativeKey","ffeeddccbbaa99887766554433221100")] {
            let mut q=p.clone(); q[k]=json!(v); let mut other=restored(&q,local.clone()); assert!(!other.accept(control),"{k}");
        }
        let mut q=p.clone(); q["target"]["id"]=json!("other"); assert!(!restored(&q,local.clone()).accept(control));
        let mut missing=NativeSidebar::prepare(&p,"projects","",&json!({}),vec![]).unwrap(); assert!(!missing.accept(control));
    }
    #[test] fn native_sidebar_revision_membership_choices_and_hidden_lists_retire_positions() {
        let p=request(); let mut nav=ns(&p); let (_,pager)=nav.slice(rows(200),"settled","p",16,""); let control=next(&pager).to_string();
        let local=nav.local(); let mut nav=restored(&p,local); assert!(nav.accept(&control));
        let (_,page)=nav.slice(rows(200),"settled","p",16,""); assert_eq!(page["offset"],16);
        let local=nav.local();
        // Ticking clocks does not change the metadata revision.
        let mut q=p.clone(); q["now"]=json!(99999); let mut nav=restored(&q,local.clone()); assert_eq!(nav.slice(rows(200),"settled","p",16,"").1["offset"],16);
        let mut reversed=rows(200); reversed.reverse(); let mut nav=restored(&p,local.clone()); assert_eq!(nav.slice(reversed,"settled","p",16,"").1["offset"],0);
        let mut retired=restored(&p,nav.local()); assert_eq!(retired.slice(rows(200),"settled","p",16,"").1["offset"],0);
        let nav=restored(&p,local.clone()); let mut hidden=restored(&p,nav.local()); assert_eq!(hidden.slice(rows(200),"settled","p",16,"").1["offset"],0);
        let mut q=p.clone(); q["revision"]=json!(8); let mut nav=restored(&q,local.clone()); assert!(nav.accept(&control)); assert_eq!(nav.slice(rows(200),"settled","p",16,"").1["offset"],0);
        let mut q=p.clone(); q["local"]=local.clone(); let mut nav=NativeSidebar::prepare(&q,"projects","search",&json!({}),vec!["p".into()]).unwrap(); assert!(nav.accept(&control)); assert_eq!(nav.slice(rows(200),"settled","p",16,"").1["offset"],0);
        let mut nav=restored(&p,local); assert_eq!(nav.slice(rows(200),"settled","p",8,"").1["offset"],0);
    }
    #[test] fn native_sidebar_only_hub_budgets_and_empty_lists_have_no_pager() {
        let mut p=request(); p["projects"]=json!(99); p["rows"]=json!(99); let mut nav=ns(&p);
        assert_eq!((nav.project_limit(),nav.row_budget()),(8,48)); assert!(nav.slice(vec![],"threads","p",16,"").1.is_null());
        let mut local=nav.local(); local["local.native.sidebar.budget"]=json!("{\"projects\":4,\"rows\":24}");
        let nav=restored(&p,local); assert_eq!((nav.project_limit(),nav.row_budget()),(4,24));
        p["nativeKey"]=json!("zz112233445566778899aabbccddeeff00"); assert!(NativeSidebar::prepare(&p,"projects","",&json!({}),vec![]).is_none());
        p=request(); p["nativeSidebar"]=json!(true); assert!(NativeSidebar::prepare(&p,"projects","",&json!({}),vec![]).is_none());
    }
}
