// backplane-step2glb: a STEP file to a GLB with OpenCascade, natively (no FreeCAD, no Bun, no Python).
//
//   backplane-step2glb IN.step OUT.glb [--linear MM] [--relative] [--angular RAD] [--parts DIR]
//   (--relative: --linear is a share of each edge's size, not millimetres)
//   [--fine x0,y0,z0,x1,y1,z1 --fine-linear L --fine-angular A [--fine-min MM]]: the solids that meet that box (millimetres) are meshed at L
//   and A instead, no segment under MM (finer than the wire's 10 um corners carry, a facet's normal is noise)
//
// Reads IN through XCAF (STEPCAFControl_Reader, colours on), meshes every distinct solid once with BRepMesh (several solids at a time on
// the machine's cores, each solid's faces in parallel when there are few), and writes OUT: millimetres, Z up, one mesh whose primitives
// are one colour each (a STEP's colour is linear light already in OCCT; glTF's baseColorFactor is linear light), as the FreeCAD scripts
// (native/host/src/files/scripts/freecad_gui.py) write it, so the hub reads both the same. Colours: a face's own, else its solid's, else a
// tint a solid from the palette the FreeCAD script uses (FreeCAD's default grey-blue counts as no colour, as it does there).
//
// --parts DIR: the model streams. Each batch of solids, the moment its meshes are done, is written as DIR/NNNN.glb (the same format) and
// said on stdout as `PART <triangles> <leaves> <file>` (the file last: its path may hold spaces); OUT is written after them all. Before the first: `OF <solids> <faces>`.
// Always on stdout: `TIME <stage> <ms>` per stage (read, transfer, mesh, write) and, last, `TRIS <triangles>`. Exit 0 only when OUT was
// written with at least one triangle; else a line saying why and exit 1 (2: bad arguments, 3: unreadable STEP).
#include <BRepBndLib.hxx>
#include <BRepMesh_IncrementalMesh.hxx>
#include <Bnd_Box.hxx>
#include <BRep_Builder.hxx>
#include <IMeshTools_Parameters.hxx>
#include <Interface_Static.hxx>
#include <Message.hxx>
#include <Message_Messenger.hxx>
#include <Poly_Triangle.hxx>
#include <Quantity_Color.hxx>
#include <BRep_Tool.hxx>
#include <Poly_Triangulation.hxx>
#include <ShapeProcess_OperLibrary.hxx>
#include <ShapeProcess_ShapeContext.hxx>
#include <XCAFPrs.hxx>
#include <XCAFPrs_IndexedDataMapOfShapeStyle.hxx>
#include <XCAFPrs_Style.hxx>
#include <XSAlgo_ShapeProcessor.hxx>
#include <STEPCAFControl_Reader.hxx>
#include <ShapeProcess.hxx>
#include <TDocStd_Document.hxx>
#include <TopExp_Explorer.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Compound.hxx>
#include <XCAFApp_Application.hxx>
#include <XCAFDoc_ShapeTool.hxx>
#include <XCAFPrs_DocumentExplorer.hxx>

#include <algorithm>
#include <atomic>
#include <chrono>
#include <cmath>
#include <condition_variable>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <filesystem>
#include <cstring>
#include <cstdarg>
#include <map>
#include <mutex>
#include <string>
#include <thread>
#include <unordered_map>
#include <unordered_set>
#include <vector>

using Clock = std::chrono::steady_clock;
static double since(Clock::time_point t) { return std::chrono::duration<double, std::milli>(Clock::now() - t).count(); }
#if defined(__GNUC__) || defined(__clang__)
static void say(const char* fmt, ...) __attribute__((format(printf, 1, 2)));
#endif
static void say(const char* fmt, ...) { va_list a; va_start(a, fmt); vprintf(fmt, a); va_end(a); fputc('\n', stdout); fflush(stdout); }

// sRGB display value <-> linear light, per channel (glTF's rule; FreeCAD's script uses the same `lin`)
static double lin(double x) { return x <= 0.04045 ? x / 12.92 : std::pow((x + 0.055) / 1.055, 2.4); }
static double srgb(double x) { return x <= 0.0031308 ? x * 12.92 : 1.055 * std::pow(x, 1.0 / 2.4) - 0.055; }

// The palette the FreeCAD script tints an uncoloured solid from (display sRGB), and FreeCAD's default colour.
static const double PAL[6][3] = {{0.62, 0.66, 0.72}, {0.74, 0.68, 0.58}, {0.52, 0.62, 0.52}, {0.7, 0.58, 0.64}, {0.58, 0.7, 0.72}, {0.8, 0.8, 0.8}};
static const double FC_DEFAULT[3] = {0.8, 0.8, 0.901961};

// A colour as linear light, rounded to a key so equal colours share a primitive.
struct Rgb { float r, g, b; };
static uint32_t key_of(Rgb c) { auto q = [](float v) { return (uint32_t)std::lround(std::clamp(v, 0.f, 1.f) * 1023.f); }; return q(c.r) << 20 | q(c.g) << 10 | q(c.b); }
static Rgb from_srgb(const double* s) { return Rgb{(float)lin(s[0]), (float)lin(s[1]), (float)lin(s[2])}; }
// Whether a linear-light colour shows as FreeCAD's default (it means: nobody chose a colour)
static bool is_default(const Quantity_Color& c) {
  double v[3] = {c.Red(), c.Green(), c.Blue()};
  for (int k = 0; k < 3; k++) if (std::fabs(srgb(v[k]) - FC_DEFAULT[k]) >= 0.01) return false;
  return true;
}

// One colour's triangles: positions (mm, xyz) and indices, and the first vertex of each B-rep face in it (`extras.faces`: a face's
// triangles use only its own vertices, so a reader knows each triangle's face, and smooths normals within a face, crisp across faces)
struct Group { Rgb c; std::vector<float> p; std::vector<uint32_t> i; std::vector<uint32_t> faces; };
struct Model {
  std::vector<Group> groups; std::unordered_map<uint32_t, size_t> at;
  Group& of(Rgb c) { uint32_t k = key_of(c); auto f = at.find(k); if (f != at.end()) return groups[f->second]; at[k] = groups.size(); groups.push_back(Group{c, {}, {}}); return groups.back(); }
  size_t tris() const { size_t n = 0; for (auto& g : groups) n += g.i.size() / 3; return n; }
};

// The GLB the hub reads (kicad/glb.rs): one buffer, POSITION (float) and indices (uint32) a primitive, one material a colour.
static bool write_glb(const Model& m, const std::string& path, const std::string& name) {
  std::string bin, js; char tmp[512];
  std::string views, accs, mats, prims; int nv = 0, na = 0, nm = 0;
  auto num = [&](double v) { snprintf(tmp, sizeof tmp, "%.9g", std::isfinite(v) ? v : 0.0); return std::string(tmp); };
  for (auto& g : m.groups) {
    if (g.i.empty()) continue;
    float lo[3] = {INFINITY, INFINITY, INFINITY}, hi[3] = {-INFINITY, -INFINITY, -INFINITY};
    for (size_t k = 0; k < g.p.size(); k++) { lo[k % 3] = std::min(lo[k % 3], g.p[k]); hi[k % 3] = std::max(hi[k % 3], g.p[k]); }
    size_t off = bin.size(); bin.append((const char*)g.p.data(), g.p.size() * 4);
    if (nv) { views += ","; }
    views += "{\"buffer\":0,\"byteOffset\":" + std::to_string(off) + ",\"byteLength\":" + std::to_string(g.p.size() * 4) + ",\"target\":34962}"; nv++;
    if (na) { accs += ","; }
    accs += "{\"bufferView\":" + std::to_string(nv - 1) + ",\"componentType\":5126,\"count\":" + std::to_string(g.p.size() / 3) + ",\"type\":\"VEC3\",\"min\":[" + num(lo[0]) + "," + num(lo[1]) + "," + num(lo[2]) + "],\"max\":[" + num(hi[0]) + "," + num(hi[1]) + "," + num(hi[2]) + "]}"; na++;
    off = bin.size(); bin.append((const char*)g.i.data(), g.i.size() * 4);
    views += ",{\"buffer\":0,\"byteOffset\":" + std::to_string(off) + ",\"byteLength\":" + std::to_string(g.i.size() * 4) + ",\"target\":34963}"; nv++;
    accs += ",{\"bufferView\":" + std::to_string(nv - 1) + ",\"componentType\":5125,\"count\":" + std::to_string(g.i.size()) + ",\"type\":\"SCALAR\"}"; na++;
    if (nm) { mats += ","; }
    mats += "{\"pbrMetallicRoughness\":{\"baseColorFactor\":[" + num(g.c.r) + "," + num(g.c.g) + "," + num(g.c.b) + ",1],\"metallicFactor\":0,\"roughnessFactor\":0.8}}"; nm++;
    std::string fs; for (size_t k = 0; k < g.faces.size(); k++) { if (k) fs += ","; fs += std::to_string(g.faces[k]); }
    if (!prims.empty()) { prims += ","; }
    prims += "{\"attributes\":{\"POSITION\":" + std::to_string(na - 2) + "},\"indices\":" + std::to_string(na - 1) + ",\"material\":" + std::to_string(nm - 1) + ",\"extras\":{\"faces\":[" + fs + "]}}";
  }
  if (prims.empty()) return false;
  std::string nm_esc; for (char c : name) { if (c == '"' || c == '\\') nm_esc += '\\'; if ((unsigned char)c >= 0x20) nm_esc += c; }
  js = "{\"asset\":{\"version\":\"2.0\",\"generator\":\"backplane-step2glb (OpenCascade)\",\"extras\":{\"unit\":\"mm\",\"up\":\"Z\"}},\"buffers\":[{\"byteLength\":" + std::to_string(bin.size()) + "}],\"bufferViews\":[" + views + "],\"accessors\":[" + accs + "],\"materials\":[" + mats + "],\"meshes\":[{\"primitives\":[" + prims + "]}],\"nodes\":[{\"name\":\"" + nm_esc + "\",\"mesh\":0}],\"scenes\":[{\"nodes\":[0]}],\"scene\":0}";
  while (js.size() % 4) js += ' ';
  while (bin.size() % 4) bin += '\0';
  std::string part = path + ".part";
  FILE* f = fopen(part.c_str(), "wb"); if (!f) return false;
  auto u32 = [&](uint32_t v) { fwrite(&v, 4, 1, f); };
  u32(0x46546C67); u32(2); u32((uint32_t)(12 + 8 + js.size() + 8 + bin.size()));
  u32((uint32_t)js.size()); u32(0x4E4F534A); fwrite(js.data(), 1, js.size(), f);
  u32((uint32_t)bin.size()); u32(0x004E4942); fwrite(bin.data(), 1, bin.size(), f);
  bool ok = fflush(f) == 0 && !ferror(f); ok = fclose(f) == 0 && ok;
  // std::filesystem::rename replaces an existing file on every system (C's rename does not on Windows)
  std::error_code ec;
  if (ok) std::filesystem::rename(part, path, ec);
  if (!ok || ec) { std::remove(part.c_str()); return false; }
  return true;
}

// A distinct solid: as read (no healing yet), the colours its faces carry in the STEP (by face), then healed, with those colours moved
// onto the healed faces, then meshed.
struct Solid {
  TopoDS_Shape raw, healed; TDF_Label label; size_t faces = 0; bool fine = false;
  std::unordered_map<const void*, Rgb> raw_colour, colour;  // by face TShape
};
// A leaf of the assembly: where it is, which solid it shows, and its colour where its faces have none (its own, else a tint).
struct Leaf { TopLoc_Location at; size_t solid; Rgb tint; };

static Rgb rgb(const Quantity_Color& q) { return Rgb{(float)q.Red(), (float)q.Green(), (float)q.Blue()}; }
// The colours a solid's faces carry (XCAF's sub-shape styles), by the face; a style on the whole solid is the leaf's, not a face's.
static void face_colours(Solid& s) {
  XCAFPrs_IndexedDataMapOfShapeStyle st; XCAFPrs::CollectStyleSettings(s.label, TopLoc_Location(), st);
  // bigger sub-shapes first, faces last: a face's own colour wins over its shell's
  for (int pass = 0; pass < 2; pass++)
    for (XCAFPrs_IndexedDataMapOfShapeStyle::Iterator it(st); it.More(); it.Next()) {
      const TopoDS_Shape& k = it.Key(); const XCAFPrs_Style& y = it.Value();
      if (k.TShape() == s.raw.TShape() || !y.IsSetColorSurf() || is_default(y.GetColorSurf())) continue;
      if ((k.ShapeType() == TopAbs_FACE) != (pass == 1)) continue;
      for (TopExp_Explorer f(k, TopAbs_FACE); f.More(); f.Next()) s.raw_colour[f.Current().TShape().get()] = rgb(y.GetColorSurf());
    }
}
// Healing a solid as the STEP reader does (its parameters, FixShape), on any thread; the faces' colours follow them to what they became.
static void heal(Solid& s, const XSAlgo_ShapeProcessor::ParameterMap& params, const ShapeProcess::OperationsFlags& ops) {
  XSAlgo_ShapeProcessor proc(params);
  TopoDS_Shape h = proc.ProcessShape(s.raw, ops, Message_ProgressRange());
  s.healed = h.IsNull() ? s.raw : h;
  if (s.raw_colour.empty()) return;
  Handle(ShapeProcess_ShapeContext) ctx = proc.GetContext();
  for (TopExp_Explorer f(s.raw, TopAbs_FACE); f.More(); f.Next()) {
    auto c = s.raw_colour.find(f.Current().TShape().get()); if (c == s.raw_colour.end()) continue;
    const TopoDS_Shape* now = (!ctx.IsNull() && ctx->Map().IsBound(f.Current())) ? &ctx->Map().Find(f.Current()) : &f.Current();
    for (TopExp_Explorer g(*now, TopAbs_FACE); g.More(); g.Next()) s.colour[g.Current().TShape().get()] = c->second;
  }
}
// A leaf's triangles into a model, at their place in the assembly, each face in its colour.
static void add_leaf(Model& m, const Leaf& l, const Solid& s) {
  TopoDS_Shape at = s.healed.Located(l.at);
  for (TopExp_Explorer e(at, TopAbs_FACE); e.More(); e.Next()) {
    const TopoDS_Face& f = TopoDS::Face(e.Current());
    TopLoc_Location fl; Handle(Poly_Triangulation) tr = BRep_Tool::Triangulation(f, fl);
    if (tr.IsNull() || tr->NbTriangles() == 0) continue;
    auto c = s.colour.find(f.TShape().get());
    Group& g = m.of(c == s.colour.end() ? l.tint : c->second);
    const gp_Trsf T = fl.Transformation(); const bool flip = f.Orientation() == TopAbs_REVERSED;
    const uint32_t base = (uint32_t)(g.p.size() / 3); const int n = tr->NbNodes();
    g.faces.push_back(base);
    for (int i = 1; i <= n; i++) { gp_Pnt p = tr->Node(i).Transformed(T); g.p.push_back((float)p.X()); g.p.push_back((float)p.Y()); g.p.push_back((float)p.Z()); }
    for (int t = 1; t <= tr->NbTriangles(); t++) {
      int a, b, c3; tr->Triangle(t).Get(a, b, c3); if (flip) std::swap(b, c3);
      if (a < 1 || b < 1 || c3 < 1 || a > n || b > n || c3 > n) continue;
      g.i.push_back(base + a - 1); g.i.push_back(base + b - 1); g.i.push_back(base + c3 - 1);
    }
  }
}

int main(int argc, char** argv) {
  if (argc < 3) { say("usage: backplane-step2glb IN.step OUT.glb [--linear MM|SHARE] [--relative] [--angular RAD] [--parts DIR]"); return 2; }
  std::string in = argv[1], out = argv[2], parts; double linear = 0.03, angular = 0.2; bool relative = false;
  double fine_box[6] = {0, 0, 0, 0, 0, 0}, fine_linear = 0.001, fine_angular = 0.05, fine_min = 0; bool fine = false;
  for (int i = 3; i < argc; i++) {
    std::string a = argv[i];
    if (a == "--relative") { relative = true; continue; }
    if (i + 1 >= argc) { say("%s needs a value", a.c_str()); return 2; }
    if (a == "--fine") { fine = sscanf(argv[++i], "%lf,%lf,%lf,%lf,%lf,%lf", &fine_box[0], &fine_box[1], &fine_box[2], &fine_box[3], &fine_box[4], &fine_box[5]) == 6; if (!fine) { say("--fine needs x0,y0,z0,x1,y1,z1"); return 2; } continue; }
    if (a == "--fine-linear") { fine_linear = atof(argv[++i]); continue; }
    if (a == "--fine-angular") { fine_angular = atof(argv[++i]); continue; }
    if (a == "--fine-min") { fine_min = atof(argv[++i]); continue; }
    if (a == "--linear") linear = atof(argv[++i]); else if (a == "--angular") angular = atof(argv[++i]); else if (a == "--parts") parts = argv[++i];
    else { say("unknown argument %s", a.c_str()); return 2; }
  }
  if (!(linear > 0) || !(angular > 0)) { say("--linear and --angular must be positive"); return 2; }
  // OpenCascade's own messages (its reader's warnings) stay off stdout, which is this program's protocol
  Message::DefaultMessenger()->RemovePrinters(STANDARD_TYPE(Message_Printer));

  auto t = Clock::now();
  Handle(TDocStd_Document) doc; XCAFApp_Application::GetApplication()->NewDocument("MDTV-XCAF", doc);
  STEPCAFControl_Reader r; r.SetColorMode(true); r.SetNameMode(false); r.SetLayerMode(false); r.SetPropsMode(false); r.SetGDTMode(false); r.SetMatMode(false); r.SetViewMode(false);
  Interface_Static::SetCVal("xstep.cascade.unit", "MM");
  if (r.ReadFile(in.c_str()) != IFSelect_RetDone) { say("could not read %s as STEP", in.c_str()); return 3; }
  say("TIME read %.0f", since(t)); t = Clock::now();
  // The reader heals each solid as it takes it out, one after another: most of a big file's time. It takes them out unhealed here, and
  // they are healed below, several at once, with the reader's own parameters.
  r.ChangeReader().SetShapeProcessFlags(ShapeProcess::OperationsFlags());
  if (!r.Transfer(doc)) { say("could not take the shapes out of %s", in.c_str()); return 3; }
  XSAlgo_ShapeProcessor::ParameterMap params = r.ChangeReader().GetShapeFixParameters();
  ShapeProcess::OperationsFlags ops; ops.set(ShapeProcess::FixShape);
  say("TIME transfer %.0f", since(t)); t = Clock::now();

  // the leaves, and the distinct solids they show (an instanced part is healed and meshed once)
  std::vector<Leaf> leaves; std::vector<Solid> solids; std::unordered_map<const void*, size_t> solid_ix;
  size_t faces = 0, plain = 0;
  for (XCAFPrs_DocumentExplorer ex(doc, XCAFPrs_DocumentExplorerFlags_OnlyLeafNodes); ex.More(); ex.Next()) {
    const XCAFPrs_DocumentNode& n = ex.Current();
    TopoDS_Shape s = XCAFDoc_ShapeTool::GetShape(n.RefLabel);
    if (s.IsNull()) continue;
    TopExp_Explorer has(s, TopAbs_FACE); if (!has.More()) continue;
    auto k = solid_ix.find(s.TShape().get());
    size_t ix;
    if (k == solid_ix.end()) {
      ix = solids.size(); solid_ix[s.TShape().get()] = ix; solids.emplace_back();
      Solid& so = solids.back(); so.raw = s.Located(TopLoc_Location()); so.label = n.RefLabel;
      for (TopExp_Explorer f(s, TopAbs_FACE); f.More(); f.Next()) so.faces++;
      faces += so.faces; face_colours(so);
    } else ix = k->second;
    // the leaf's own colour, else a tint (as FreeCAD's script: one a solid without a colour, in turn)
    Rgb tint = from_srgb(PAL[plain % 6]);
    if (n.Style.IsSetColorSurf() && !is_default(n.Style.GetColorSurf())) tint = rgb(n.Style.GetColorSurf()); else plain++;
    leaves.push_back(Leaf{n.Location, ix, tint});
    // --fine: a solid with a leaf in the box is meshed finer (the part the person zoomed in on)
    if (fine && !solids[ix].fine) {
      Bnd_Box bb; BRepBndLib::Add(s.Located(n.Location), bb, false);
      if (!bb.IsVoid()) { double x0, y0, z0, x1, y1, z1; bb.Get(x0, y0, z0, x1, y1, z1);
        solids[ix].fine = x1 >= fine_box[0] && x0 <= fine_box[3] && y1 >= fine_box[1] && y0 <= fine_box[4] && z1 >= fine_box[2] && z0 <= fine_box[5]; }
    }
  }
  if (leaves.empty()) { say("no surfaces in %s", in.c_str()); return 1; }
  if (!parts.empty()) say("OF %zu %zu", leaves.size(), faces);
  if (fine) { size_t nf = 0; for (auto& so : solids) nf += so.fine; say("FINE %zu", nf); }

  // Two solids that share a face cannot be healed or meshed at once (the face is one); then one worker does them in turn.
  bool shared = false;
  if (solids.size() > 1) {
    std::unordered_set<const void*> seen;
    for (auto& s : solids) { std::unordered_set<const void*> own; for (TopExp_Explorer f(s.raw, TopAbs_FACE); f.More(); f.Next()) own.insert(f.Current().TShape().get()); for (auto p : own) if (!seen.insert(p).second) { shared = true; break; } if (shared) break; }
  }
  ShapeProcess_OperLibrary::Init();  // the healing operators' registry, filled once before any worker reads it
  const unsigned cores = std::max(1u, std::thread::hardware_concurrency());
  // --relative: the linear deflection is a share of each edge's size (OpenCascade's relative mode), so a small hole is as round as a big one
  IMeshTools_Parameters p; p.Deflection = linear; p.Angle = angular; p.Relative = relative;
  // each solid's faces in parallel too when there are fewer solids than cores (one big part: all cores on it)
  p.InParallel = shared || solids.size() < cores;
  std::vector<char> meshed(solids.size(), 0);
  std::mutex mu; std::condition_variable cv;
  // what the workers read: it lives as long as they do. The biggest solids first: the longest finishes soonest.
  std::vector<size_t> order(solids.size()); for (size_t i = 0; i < order.size(); i++) order[i] = i;
  std::stable_sort(order.begin(), order.end(), [&](size_t a, size_t b) { return solids[a].faces > solids[b].faces; });
  std::atomic<size_t> next{0};
  auto work = [&] {
    for (size_t k; (k = next.fetch_add(1)) < order.size();) {
      Solid& s = solids[order[k]];
      heal(s, params, ops);
      IMeshTools_Parameters sp = p; if (s.fine) { sp.Deflection = fine_linear; sp.Angle = fine_angular; if (fine_min > 0) sp.MinSize = fine_min; }
      BRepMesh_IncrementalMesh(s.healed, sp);
      std::lock_guard<std::mutex> g(mu); meshed[order[k]] = 1; cv.notify_all();
    }
  };
  std::vector<std::thread> workers;
  unsigned nw = shared ? 1u : (unsigned)std::min<size_t>(cores, solids.size());
  for (unsigned w = 0; w < nw; w++) workers.emplace_back(work);

  // the stream: each leaf whose solid is meshed, in batches; the whole model at the end
  Model all; size_t sent = 0, batch_no = 0;
  std::vector<bool> out_yet(leaves.size(), false);
  auto t_first = Clock::now(); bool first = true;
  Model batch; size_t batch_leaves = 0; auto batch_t = Clock::now();
  auto flush = [&](bool force) {
    if (batch_leaves == 0) return;
    size_t bt = batch.tris();
    if (!force && bt < 20000 && since(batch_t) < 60) return;
    char name[32]; snprintf(name, sizeof name, "%04zu.glb", batch_no++);
    std::string path = parts + "/" + name;
    if (write_glb(batch, path, name)) { say("PART %zu %zu %s", bt, batch_leaves, path.c_str()); if (first) { say("TIME first %.0f", since(t_first)); first = false; } }
    batch = Model(); batch_leaves = 0; batch_t = Clock::now();
  };
  while (sent < leaves.size()) {
    std::vector<size_t> ready;
    { std::unique_lock<std::mutex> g(mu);
      cv.wait_for(g, std::chrono::milliseconds(40), [&] { for (size_t i = 0; i < leaves.size(); i++) if (!out_yet[i] && meshed[leaves[i].solid]) return true; return false; });
      for (size_t i = 0; i < leaves.size(); i++) if (!out_yet[i] && meshed[leaves[i].solid]) { out_yet[i] = true; ready.push_back(i); } }
    for (size_t i : ready) { const Solid& so = solids[leaves[i].solid]; add_leaf(all, leaves[i], so); if (!parts.empty()) { add_leaf(batch, leaves[i], so); batch_leaves++; } sent++; }
    if (!parts.empty()) flush(sent == leaves.size());
  }
  for (auto& w : workers) w.join();
  say("TIME heal+mesh %.0f", since(t)); t = Clock::now();
  std::string base = in.substr(in.find_last_of("/\\") + 1);
  size_t tris = all.tris();
  if (tris == 0) { say("could not mesh %s (no triangles)", in.c_str()); return 1; }
  if (!write_glb(all, out, base)) { say("could not write %s", out.c_str()); return 1; }
  say("TIME write %.0f", since(t));
  say("TRIS %zu", tris);
  // done: the document's teardown takes a fifth of a second on a board and changes nothing on disk
  fflush(stdout); std::_Exit(0);
}
