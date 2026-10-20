// Backplane fonts (FreeType + fontconfig)
// =======================================
//
// System fonts, anti-aliased. Both libraries are opened at run time
// (dlopen), so the binary links neither and needs no headers: the few
// FreeType structs we read are declared below with the public 2.x layout
// (stable since 2.0; checked against FreeType 2.14's headers with
// offsetof). A machine without libfreetype.so.6 gets ENOTSUP from
// Font.open and the app keeps its bitmap font.
//
// Everything crosses as Base types (strings, U32, lists of U32 words);
// src/gfx/text.bend decodes them.
//
//   Font.find(pattern)        fontconfig match: (family, (file, index))
//   Font.open(path, index, px) a face id (small integer)
//   Font.metrics(face)        [ascender, descender, height, max advance]
//                             in 26.6 pixels (descender as a magnitude)
//   Font.glyph(face, cp)      [found, advance, left + 32768, top + 32768,
//                             w, h, coverage...] advance in 26.6; coverage
//                             is w*h bytes row-major, four per word, first
//                             pixel in the low byte; [] on error. A code
//                             point the face lacks (Greek, CJK, symbols in
//                             a Latin-only face) comes from the font
//                             fontconfig picks for it at the same size
//   Font.kern(face, a, b)     kerning between two code points, 26.6,
//                             biased by 32768

#include <dlfcn.h>
#include <stdint.h>
#include <string.h>
#ifdef __linux__
#include <unistd.h>
#endif

#define FX_BIAS 32768

typedef struct { long x, y; } FxVector;
typedef struct { long xMin, yMin, xMax, yMax; } FxBBox;
typedef struct { void* data; void (*finalizer)(void*); } FxGeneric;

typedef struct {
  long width, height, horiBearingX, horiBearingY, horiAdvance;
  long vertBearingX, vertBearingY, vertAdvance;
} FxGlyphMetrics;

typedef struct {
  unsigned int   rows;
  unsigned int   width;
  int            pitch;
  unsigned char* buffer;
  unsigned short num_grays;
  unsigned char  pixel_mode;
  unsigned char  palette_mode;
  void*          palette;
} FxBitmap;

typedef struct {
  unsigned short x_ppem, y_ppem;
  long           x_scale, y_scale;
  long           ascender, descender, height, max_advance;
} FxSizeMetrics;

typedef struct {
  void*         face;
  FxGeneric     generic;
  FxSizeMetrics metrics;
} FxSize;

// the prefix of FT_GlyphSlotRec we read
typedef struct {
  void*          library;
  void*          face;
  void*          next;
  unsigned int   glyph_index;
  FxGeneric      generic;
  FxGlyphMetrics metrics;
  long           linearHoriAdvance;
  long           linearVertAdvance;
  FxVector       advance;
  int            format;
  FxBitmap       bitmap;
  int            bitmap_left;
  int            bitmap_top;
} FxSlot;

// the prefix of FT_FaceRec we read
typedef struct {
  long           num_faces, face_index, face_flags, style_flags, num_glyphs;
  char*          family_name;
  char*          style_name;
  int            num_fixed_sizes;
  void*          available_sizes;
  int            num_charmaps;
  void*          charmaps;
  FxGeneric      generic;
  FxBBox         bbox;
  unsigned short units_per_EM;
  short          ascender, descender, height;
  short          max_advance_width, max_advance_height;
  short          underline_position, underline_thickness;
  FxSlot*        glyph;
  FxSize*        size;
  void*          charmap;
} FxFace;

// fontconfig's FcFontSet
typedef struct {
  int    nfont;
  int    sfont;
  void** fonts;
} FxFontSet;

#define FX_LOAD_NO_BITMAP     0x8
#define FX_LOAD_TARGET_LIGHT  0x10000
#define FX_RENDER_MODE_NORMAL 0
#define FX_PIXEL_MODE_GRAY    2
#define FX_KERNING_DEFAULT    0

#define FX_FT_FNS(X) \
  X(FT_Init_FreeType, int, (void**)) \
  X(FT_New_Face, int, (void*, const char*, long, FxFace**)) \
  X(FT_Set_Pixel_Sizes, int, (FxFace*, unsigned, unsigned)) \
  X(FT_Get_Char_Index, unsigned, (FxFace*, unsigned long)) \
  X(FT_Load_Glyph, int, (FxFace*, unsigned, int32_t)) \
  X(FT_Render_Glyph, int, (FxSlot*, int)) \
  X(FT_Get_Kerning, int, (FxFace*, unsigned, unsigned, unsigned, FxVector*))

#define FX_FC_FNS(X) \
  X(FcInitLoadConfigAndFonts, void*, (void)) \
  X(FcNameParse, void*, (const unsigned char*)) \
  X(FcConfigSubstitute, int, (void*, void*, int)) \
  X(FcDefaultSubstitute, void, (void*)) \
  X(FcFontMatch, void*, (void*, void*, int*)) \
  X(FcPatternGetString, int, (void*, const char*, int, unsigned char**)) \
  X(FcPatternGetInteger, int, (void*, const char*, int, int*)) \
  X(FcPatternDestroy, void, (void*)) \
  X(FcPatternCreate, void*, (void)) \
  X(FcPatternAddString, int, (void*, const char*, const unsigned char*)) \
  X(FcPatternAddInteger, int, (void*, const char*, int)) \
  X(FcPatternAddCharSet, int, (void*, const char*, void*)) \
  X(FcPatternGetCharSet, int, (void*, const char*, int, void**)) \
  X(FcPatternGetBool, int, (void*, const char*, int, int*)) \
  X(FcCharSetCreate, void*, (void)) \
  X(FcCharSetAddChar, int, (void*, u32)) \
  X(FcCharSetHasChar, int, (void*, u32)) \
  X(FcCharSetDestroy, void, (void*)) \
  X(FcFontSort, FxFontSet*, (void*, void*, int, void**, int*)) \
  X(FcFontSetDestroy, void, (FxFontSet*))

#define FX_PTR(name, ret, args) static __attribute__((unused)) ret (*fx_##name) args;
FX_FT_FNS(FX_PTR)
FX_FC_FNS(FX_PTR)

#define FX_SYM(name, ret, args) \
  if ((fx_##name = (ret (*) args)dlsym(lib, #name)) == NULL) { return 0; }

static void*   fx_lib = NULL;
static FxFace* fx_faces[256];
static u32     fx_nfaces = 0;
static int (*fx_FcConfigAppFontAddFile)(void*, const unsigned char*) = NULL;

// FreeType, loaded and initialised once
static __attribute__((unused)) int fx_ft_load(void) {
  static int state = 0;
  if (state != 0) {
    return state > 0;
  }
  state = -1;
  void* lib = dlopen(
#ifdef __CYGWIN__
    "cygfreetype-6.dll",
#else
    "libfreetype.so.6",
#endif
    RTLD_NOW | RTLD_LOCAL);
  if (lib == NULL) {
    return 0;
  }
  FX_FT_FNS(FX_SYM)
  if (fx_FT_Init_FreeType(&fx_lib) != 0) {
    return 0;
  }
  state = 1;
  return 1;
}

static __attribute__((unused)) void* fx_fc_load(void) {
  static int state = 0;
  static void* config = NULL;
  if (state != 0) {
    return state > 0 ? config : NULL;
  }
  state = -1;
  void* lib = dlopen(
#ifdef __CYGWIN__
    "cygfontconfig-1.dll",
#else
    "libfontconfig.so.1",
#endif
    RTLD_NOW | RTLD_LOCAL);
  if (lib == NULL) {
    return NULL;
  }
  FX_FC_FNS(FX_SYM)
  fx_FcConfigAppFontAddFile = (int (*)(void*, const unsigned char*))dlsym(lib, "FcConfigAppFontAddFile");
  config = fx_FcInitLoadConfigAndFonts();
  if (config == NULL) {
    return NULL;
  }
#ifdef __linux__
  // The package keeps its licensed cuneiform font beside the executable.
  char exe[4096];
  ssize_t n = readlink("/proc/self/exe", exe, sizeof(exe) - 1);
  if (n > 0 && n < (ssize_t)sizeof(exe) - 1) {
    exe[n] = 0;
    char* slash = strrchr(exe, '/');
    if (slash != NULL) {
      slash[1] = 0;
      static const char font[] = "fonts/NotoSansCuneiform-Regular.ttf";
      size_t dir = strlen(exe);
      if (dir + sizeof(font) <= sizeof(exe) && fx_FcConfigAppFontAddFile != NULL) {
        memcpy(exe + dir, font, sizeof(font));
        fx_FcConfigAppFontAddFile(config, (const unsigned char*)exe);
      }
    }
  }
#endif
  state = 1;
  return config;
}

static __attribute__((unused)) FxFace* fx_face(u32 id) {
  return id < fx_nfaces ? fx_faces[id] : NULL;
}

// the n words as a Bend List<U32>
static __attribute__((unused)) Term fx_list(Env e, const u32* ws, u64 n) {
  Term xs = term_pak(CID_NIL, 0);
  for (u64 i = n; i > 0; i -= 1) {
    xs = io_node(e, CID_CON, (Term)ws[i - 1], xs);
  }
  return xs;
}

#ifdef CID(Font.find)

// the best system font for a fontconfig pattern ("DejaVu Sans Mono",
// "monospace:weight=bold"): (family, (file, index))
Term font_find_run(Env e, Term* f, IoWork* w) {
  u64 n = 0;
  char* pat = io_cstr(e, f[0], &n);
  void* config = fx_fc_load();
  if (config == NULL) {
    free(pat);
    return io_fail(e, ENOTSUP, "fontconfig (libfontconfig.so.1) is not available");
  }
  void* p = fx_FcNameParse((const unsigned char*)pat);
  free(pat);
  if (p == NULL) {
    return io_fail(e, EINVAL, "bad font pattern");
  }
  fx_FcConfigSubstitute(config, p, 0);
  fx_FcDefaultSubstitute(p);
  int res = 0;
  void* m = fx_FcFontMatch(config, p, &res);
  fx_FcPatternDestroy(p);
  unsigned char* file = NULL;
  unsigned char* family = NULL;
  int index = 0;
  if (m == NULL || fx_FcPatternGetString(m, "file", 0, &file) != 0 || file == NULL) {
    if (m) {
      fx_FcPatternDestroy(m);
    }
    return io_fail(e, ENOENT, "no font matches");
  }
  if (fx_FcPatternGetString(m, "family", 0, &family) != 0 || family == NULL) {
    family = (unsigned char*)"";
  }
  fx_FcPatternGetInteger(m, "index", 0, &index);
  Term r = io_tup(e, io_str(e, (char*)family, strlen((char*)family)),
    io_tup(e, io_str(e, (char*)file, strlen((char*)file)), (Term)(u32)index));
  fx_FcPatternDestroy(m);
  return io_done(e, r);
}

static void __attribute__((constructor)) font_find_use(void) {
  io_eff(CID(Font.find), font_find_run);
}

#endif

#ifdef CID(Font.open)

// a face of the font file at px pixels per em
Term font_open_run(Env e, Term* f, IoWork* w) {
  u64 n = 0;
  char* path = io_cstr(e, f[0], &n);
  long index = (long)(u32)f[1];
  u32 px = (u32)f[2];
  if (!fx_ft_load()) {
    free(path);
    return io_fail(e, ENOTSUP, "FreeType (libfreetype.so.6) is not available");
  }
  if (fx_nfaces >= 256) {
    free(path);
    return io_fail(e, EMFILE, "too many font faces");
  }
  FxFace* face = NULL;
  int err = fx_FT_New_Face(fx_lib, path, index, &face);
  free(path);
  if (err != 0 || face == NULL) {
    return io_fail(e, ENOENT, "FreeType cannot open the font");
  }
  if (fx_FT_Set_Pixel_Sizes(face, 0, px ? px : 13) != 0) {
    return io_fail(e, EINVAL, "FreeType cannot size the font");
  }
  fx_faces[fx_nfaces] = face;
  fx_nfaces += 1;
  return io_done(e, (Term)(fx_nfaces - 1));
}

static void __attribute__((constructor)) font_open_use(void) {
  io_eff(CID(Font.open), font_open_run);
}

#endif

#ifdef CID(Font.metrics)

Term font_metrics_run(Env e, Term* f, IoWork* w) {
  FxFace* face = fx_face((u32)f[0]);
  if (face == NULL) {
    return fx_list(e, NULL, 0);
  }
  FxSizeMetrics* m = &face->size->metrics;
  long desc = m->descender < 0 ? -m->descender : m->descender;
  u32 ws[4] = { (u32)m->ascender, (u32)desc, (u32)m->height, (u32)m->max_advance };
  return fx_list(e, ws, 4);
}

static void __attribute__((constructor)) font_metrics_use(void) {
  io_eff(CID(Font.metrics), font_metrics_run);
}

#endif

#ifdef CID(Font.glyph)

// Fallback faces
// --------------
// Faces opened for code points the UI's faces lack, kept for the rest of
// the run: a glyph first tries the ones already open at its size and
// style, then asks fontconfig (FcFontSort over a pattern that wants the
// code point) and opens the first outline font that has it.

#define FX_FB_MAX 48
#define FX_STYLE_ITALIC 1
#define FX_STYLE_BOLD 2
#define FX_FACE_FIXED_WIDTH 4

typedef struct {
  FxFace* face;
  u32     px;
  long    style;
  FxFace* primary; // fixed/proportional policy, cell and line-box metrics
} FxFallback;

static FxFallback fx_fb[FX_FB_MAX];
static u32        fx_nfb = 0;

// the glyph index of cp in f when it loads as an outline (0: it does not)
static unsigned fx_has(FxFace* f, u32 cp) {
  unsigned gi = fx_FT_Get_Char_Index(f, cp);
  if (gi == 0 || fx_FT_Load_Glyph(f, gi, FX_LOAD_TARGET_LIGHT | FX_LOAD_NO_BITMAP) != 0) {
    return 0;
  }
  return gi;
}

// Read preference once. Locale precedence follows LC_ALL, LC_MESSAGES,
// LANG; a non-CJK locale deliberately defaults to Japanese glyph forms.
static const char* fx_cjk_lang(void) {
  static const char* lang = NULL;
  if (lang != NULL) return lang;
  const char* override = getenv("BACKPLANE_CJK_LANG");
  const char* loc = override;
  char tag[64];
  for (int attempt = 0; attempt < 2; attempt += 1) {
    if (attempt || loc == NULL || !*loc) {
      loc = getenv("LC_ALL");
      if (loc == NULL || !*loc) loc = getenv("LC_MESSAGES");
      if (loc == NULL || !*loc) loc = getenv("LANG");
    }
    unsigned i = 0;
    while (loc && loc[i] && loc[i] != '.' && loc[i] != '@' && i < sizeof(tag) - 1) {
      char ch = loc[i];
      tag[i++] = ch == '_' ? '-' : (ch >= 'A' && ch <= 'Z' ? ch + 'a' - 'A' : ch);
    }
    tag[i] = 0;
    if (override && *override && !attempt) {
      if (!strcmp(tag, "ja") || !strcmp(tag, "zh-cn") || !strcmp(tag, "zh-tw") || !strcmp(tag, "ko")) break;
    } else break;
  }
  lang = "ja";
  if (!strncmp(tag, "ko", 2) && (!tag[2] || tag[2] == '-')) lang = "ko";
  if (!strncmp(tag, "zh", 2) && (!tag[2] || tag[2] == '-')) {
    lang = strstr(tag, "-tw") || strstr(tag, "-hk") || strstr(tag, "-mo") || strstr(tag, "-hant") ? "zh-tw" : "zh-cn";
  }
  return lang;
}

// Same ranges as core/cjk.bend and Vt.wide (no Unicode library needed).
static int fx_cjk_wide(u32 cp) {
  static const u32 ranges[][2] = {
    {4352,4447}, {11904,12350}, {12353,13311}, {13312,19903},
    {19968,40959}, {40960,42191}, {44032,55203}, {63744,64255},
    {65072,65103}, {65280,65376}, {65504,65510}, {127744,128591},
    {128640,128767}, {129280,129535}, {131072,262141}
  };
  for (unsigned i = 0; i < sizeof(ranges) / sizeof(ranges[0]); i += 1)
    if (cp >= ranges[i][0] && cp <= ranges[i][1]) return 1;
  return 0;
}

static FxFace* fx_fb_open(void* pat, u32 cp, u32 px, long style, FxFace* primary) {
  unsigned char* file = NULL;
  int index = 0;
  int color = 0;
  void* cs = NULL;
  if (fx_FcPatternGetBool(pat, "color", 0, &color) == 0 && color) {
    return NULL;
  }
  if (fx_FcPatternGetCharSet(pat, "charset", 0, &cs) != 0 || cs == NULL || !fx_FcCharSetHasChar(cs, cp)) {
    return NULL;
  }
  if (fx_FcPatternGetString(pat, "file", 0, &file) != 0 || file == NULL) {
    return NULL;
  }
  fx_FcPatternGetInteger(pat, "index", 0, &index);
  FxFace* f = NULL;
  if (fx_FT_New_Face(fx_lib, (const char*)file, index, &f) != 0 || f == NULL) {
    return NULL;
  }
  if (fx_FT_Set_Pixel_Sizes(f, 0, px) != 0 || fx_has(f, cp) == 0 || fx_nfb >= FX_FB_MAX) {
    return NULL;
  }
  fx_fb[fx_nfb].face = f;
  fx_fb[fx_nfb].px = px;
  fx_fb[fx_nfb].style = style;
  fx_fb[fx_nfb].primary = primary;
  fx_nfb += 1;
  return f;
}

// a face that has cp, for text set in face (NULL: none on this machine)
static FxFace* fx_fallback(FxFace* face, u32 cp) {
  u32 px = face->size->metrics.y_ppem;
  long style = face->style_flags & (FX_STYLE_ITALIC | FX_STYLE_BOLD);
  for (u32 i = 0; i < fx_nfb; i += 1) {
    if (fx_fb[i].primary == face && fx_fb[i].px == px && fx_fb[i].style == style && fx_has(fx_fb[i].face, cp) != 0) {
      return fx_fb[i].face;
    }
  }
  void* config = fx_fc_load();
  if (config == NULL || fx_nfb >= FX_FB_MAX) {
    return NULL;
  }
  void* p = fx_FcPatternCreate();
  void* cs = fx_FcCharSetCreate();
  if (p == NULL || cs == NULL) {
    if (p) {
      fx_FcPatternDestroy(p);
    }
    if (cs) {
      fx_FcCharSetDestroy(cs);
    }
    return NULL;
  }
  fx_FcCharSetAddChar(cs, cp);
  fx_FcPatternAddString(p, "lang", (const unsigned char*)fx_cjk_lang());
  fx_FcPatternAddCharSet(p, "charset", cs);
  fx_FcPatternAddString(p, "family", (const unsigned char*)((face->face_flags & FX_FACE_FIXED_WIDTH) ? "monospace" : "sans-serif"));
  if (style & FX_STYLE_BOLD) {
    fx_FcPatternAddInteger(p, "weight", 200);
  }
  if (style & FX_STYLE_ITALIC) {
    fx_FcPatternAddInteger(p, "slant", 100);
  }
  fx_FcConfigSubstitute(config, p, 0);
  fx_FcDefaultSubstitute(p);
  int res = 0;
  FxFontSet* fs = fx_FcFontSort(config, p, 1, NULL, &res);
  FxFace* found = NULL;
  if (fs != NULL) {
    for (int i = 0; i < fs->nfont && i < 24 && found == NULL; i += 1) {
      found = fx_fb_open(fs->fonts[i], cp, px, style, face);
    }
    fx_FcFontSetDestroy(fs);
  }
  fx_FcPatternDestroy(p);
  fx_FcCharSetDestroy(cs);
  return found;
}

// FreeType's coverage is linear, and the rasterizer blends in sRGB, so
// light text on a dark ground comes out thin. Browsers correct for it
// (Skia's contrast / gamma hack); this lifts mid coverage the same way,
// about a 1.3 gamma, leaving 0 and 255 alone.
static u32 fx_boost(u32 c) {
  return c + c * (255 - c) * 3 / 2550;
}

// one glyph, hinted lightly (vertical only, like browsers on Linux) and
// rendered as 8-bit grayscale coverage
Term font_glyph_run(Env e, Term* f, IoWork* w) {
  FxFace* face = fx_face((u32)f[0]);
  u32 cp = (u32)f[1];
  if (face == NULL) {
    return fx_list(e, NULL, 0);
  }
  FxFace* primary = face;
  long cell = 0;
  unsigned gi = fx_FT_Get_Char_Index(face, cp);
  if (gi == 0 && (primary->face_flags & FX_FACE_FIXED_WIDTH)) {
    unsigned zero = fx_FT_Get_Char_Index(primary, '0');
    if (zero && fx_FT_Load_Glyph(primary, zero, FX_LOAD_TARGET_LIGHT | FX_LOAD_NO_BITMAP) == 0)
      cell = primary->glyph->advance.x;
  }
  if (gi == 0 && cp >= 128) {
    FxFace* fb = fx_fallback(face, cp);
    if (fb != NULL) {
      face = fb;
      gi = fx_FT_Get_Char_Index(face, cp);
    }
  }
  if (gi == 0 && fx_cjk_wide(cp)) {
    static int warned = 0;
    if (!warned) {
      fprintf(stderr, "no CJK font: install fonts-noto-cjk\n");
      warned = 1;
    }
    return fx_list(e, NULL, 0);
  }
  if (fx_FT_Load_Glyph(face, gi, FX_LOAD_TARGET_LIGHT | FX_LOAD_NO_BITMAP) != 0
    || fx_FT_Render_Glyph(face->glyph, FX_RENDER_MODE_NORMAL) != 0) {
    return fx_list(e, NULL, 0);
  }
  FxSlot* s = face->glyph;
  FxBitmap* b = &s->bitmap;
  u32 bw = b->pixel_mode == FX_PIXEL_MODE_GRAY ? b->width : 0;
  u32 bh = b->pixel_mode == FX_PIXEL_MODE_GRAY ? b->rows : 0;
  u64 nb = (u64)bw * bh;
  u64 nw = 6 + (nb + 3) / 4;
  u32* ws = io_mem(calloc(nw, 4));
  ws[0] = gi != 0;
  ws[1] = (u32)s->advance.x;
  ws[2] = (u32)(s->bitmap_left + FX_BIAS);
  if (face != primary && cell > 0) {
    long advance = cell * (fx_cjk_wide(cp) ? 2 : 1);
    ws[1] = (u32)advance;
    // Centre the advance box, preserving the font's own bearing. CJK
    // punctuation intentionally sits at an edge of its em box.
    long delta = advance - s->advance.x;
    long shift = delta >= 0 ? (delta + 64) / 128 : -((-delta + 64) / 128);
    ws[2] = (u32)(s->bitmap_left + shift + FX_BIAS);
  }
  ws[3] = (u32)(s->bitmap_top + FX_BIAS);
  ws[4] = bw;
  ws[5] = bh;
  u64 k = 0;
  for (u32 y = 0; y < bh; y += 1) {
    const unsigned char* row = b->buffer + (long)y * b->pitch;
    for (u32 x = 0; x < bw; x += 1) {
      ws[6 + k / 4] |= fx_boost(row[x]) << ((k % 4) * 8);
      k += 1;
    }
  }
  Term r = fx_list(e, ws, nw);
  free(ws);
  return r;
}

static void __attribute__((constructor)) font_glyph_use(void) {
  io_eff(CID(Font.glyph), font_glyph_run);
}

#endif

#ifdef CID(Font.kern)

Term font_kern_run(Env e, Term* f, IoWork* w) {
  FxFace* face = fx_face((u32)f[0]);
  FxVector k = { 0, 0 };
  if (face != NULL) {
    unsigned a = fx_FT_Get_Char_Index(face, (u32)f[1]);
    unsigned b = fx_FT_Get_Char_Index(face, (u32)f[2]);
    if (fx_FT_Get_Kerning(face, a, b, FX_KERNING_DEFAULT, &k) != 0) {
      k.x = 0;
    }
  }
  return (Term)(u32)(k.x + FX_BIAS);
}

static void __attribute__((constructor)) font_kern_use(void) {
  io_eff(CID(Font.kern), font_kern_run);
}

#endif
