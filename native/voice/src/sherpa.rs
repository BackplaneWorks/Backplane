//! sherpa-onnx's C API, loaded at run time from the downloaded runtime.
//!
//! The structs are written by hand against sherpa/c-api.h, v2.0.1. They go
//! by pointer, so a missing field is a silent misread: every field is here,
//! a test compares them with the C compiler's view of that header, and the
//! library's version must equal the pinned one before anything else in it is
//! called.

use crate::local::{self, Word, RATE, RUNTIME_VERSION};
use crate::session::{Decoder, Detector};
use libloading::Library;
use std::ffi::{c_char, c_void, CStr, CString};
use std::path::Path;
use std::sync::Arc;

const MIN_SPEECH: f32 = crate::session::MIN_SPEECH as f32;
const MAX_SEG: f32 = crate::session::MAX_SEG as f32;

type Str = *const c_char;

macro_rules! c_structs {
    ($($name:ident = $c:literal { $($f:ident: $t:ty,)* })*) => {
        $(
            #[repr(C)]
            #[derive(Clone, Copy)]
            pub struct $name { $(pub $f: $t,)* }
        )*
        /// Every struct: its C name, size, and each field's offset.
        pub fn layouts() -> Vec<(&'static str, usize, Vec<(&'static str, usize)>)> {
            vec![$(($c, std::mem::size_of::<$name>(), vec![$((stringify!($f), std::mem::offset_of!($name, $f)),)*]),)*]
        }
    };
}

c_structs! {
    FeatureConfig = "SherpaOnnxFeatureConfig" {
        sample_rate: i32,
        feature_dim: i32,
    }
    OfflineCtcFstDecoderConfig = "SherpaOnnxOfflineCtcFstDecoderConfig" {
        graph: Str,
        max_active: i32,
    }
    HomophoneReplacerConfig = "SherpaOnnxHomophoneReplacerConfig" {
        dict_dir: Str,
        lexicon: Str,
        rule_fsts: Str,
    }
    OfflineTransducerModelConfig = "SherpaOnnxOfflineTransducerModelConfig" {
        encoder: Str,
        decoder: Str,
        joiner: Str,
    }
    OfflineParaformerModelConfig = "SherpaOnnxOfflineParaformerModelConfig" {
        model: Str,
    }
    OfflineNemoEncDecCtcModelConfig = "SherpaOnnxOfflineNemoEncDecCtcModelConfig" {
        model: Str,
    }
    OfflineWhisperModelConfig = "SherpaOnnxOfflineWhisperModelConfig" {
        encoder: Str,
        decoder: Str,
        language: Str,
        task: Str,
        tail_paddings: i32,
        enable_token_timestamps: i32,
        enable_segment_timestamps: i32,
    }
    OfflineCanaryModelConfig = "SherpaOnnxOfflineCanaryModelConfig" {
        encoder: Str,
        decoder: Str,
        src_lang: Str,
        tgt_lang: Str,
        use_pnc: i32,
    }
    OfflineCohereTranscribeModelConfig = "SherpaOnnxOfflineCohereTranscribeModelConfig" {
        encoder: Str,
        decoder: Str,
        language: Str,
        use_punct: i32,
        use_itn: i32,
    }
    OfflineFireRedAsrModelConfig = "SherpaOnnxOfflineFireRedAsrModelConfig" {
        encoder: Str,
        decoder: Str,
    }
    OfflineFireRedAsrCtcModelConfig = "SherpaOnnxOfflineFireRedAsrCtcModelConfig" {
        model: Str,
    }
    OfflineMoonshineModelConfig = "SherpaOnnxOfflineMoonshineModelConfig" {
        preprocessor: Str,
        encoder: Str,
        uncached_decoder: Str,
        cached_decoder: Str,
        merged_decoder: Str,
    }
    OfflineTdnnModelConfig = "SherpaOnnxOfflineTdnnModelConfig" {
        model: Str,
    }
    OfflineLMConfig = "SherpaOnnxOfflineLMConfig" {
        model: Str,
        scale: f32,
    }
    OfflineSenseVoiceModelConfig = "SherpaOnnxOfflineSenseVoiceModelConfig" {
        model: Str,
        language: Str,
        use_itn: i32,
    }
    OfflineDolphinModelConfig = "SherpaOnnxOfflineDolphinModelConfig" {
        model: Str,
    }
    OfflineZipformerCtcModelConfig = "SherpaOnnxOfflineZipformerCtcModelConfig" {
        model: Str,
    }
    OfflineWenetCtcModelConfig = "SherpaOnnxOfflineWenetCtcModelConfig" {
        model: Str,
    }
    OfflineOmnilingualAsrCtcModelConfig = "SherpaOnnxOfflineOmnilingualAsrCtcModelConfig" {
        model: Str,
    }
    OfflineFunAsrNanoModelConfig = "SherpaOnnxOfflineFunASRNanoModelConfig" {
        encoder_adaptor: Str,
        llm: Str,
        embedding: Str,
        tokenizer: Str,
        system_prompt: Str,
        user_prompt: Str,
        max_new_tokens: i32,
        temperature: f32,
        top_p: f32,
        seed: i32,
        language: Str,
        itn: i32,
        hotwords: Str,
    }
    OfflineQwen3AsrModelConfig = "SherpaOnnxOfflineQwen3ASRModelConfig" {
        conv_frontend: Str,
        encoder: Str,
        decoder: Str,
        tokenizer: Str,
        max_total_len: i32,
        max_new_tokens: i32,
        temperature: f32,
        top_p: f32,
        seed: i32,
        hotwords: Str,
    }
    OfflineMedAsrCtcModelConfig = "SherpaOnnxOfflineMedAsrCtcModelConfig" {
        model: Str,
    }
    OfflineModelConfig = "SherpaOnnxOfflineModelConfig" {
        transducer: OfflineTransducerModelConfig,
        paraformer: OfflineParaformerModelConfig,
        nemo_ctc: OfflineNemoEncDecCtcModelConfig,
        whisper: OfflineWhisperModelConfig,
        tdnn: OfflineTdnnModelConfig,
        tokens: Str,
        num_threads: i32,
        debug: i32,
        provider: Str,
        model_type: Str,
        modeling_unit: Str,
        bpe_vocab: Str,
        telespeech_ctc: Str,
        sense_voice: OfflineSenseVoiceModelConfig,
        moonshine: OfflineMoonshineModelConfig,
        fire_red_asr: OfflineFireRedAsrModelConfig,
        dolphin: OfflineDolphinModelConfig,
        zipformer_ctc: OfflineZipformerCtcModelConfig,
        canary: OfflineCanaryModelConfig,
        wenet_ctc: OfflineWenetCtcModelConfig,
        omnilingual: OfflineOmnilingualAsrCtcModelConfig,
        medasr: OfflineMedAsrCtcModelConfig,
        funasr_nano: OfflineFunAsrNanoModelConfig,
        fire_red_asr_ctc: OfflineFireRedAsrCtcModelConfig,
        qwen3_asr: OfflineQwen3AsrModelConfig,
        cohere_transcribe: OfflineCohereTranscribeModelConfig,
    }
    OfflineRecognizerConfig = "SherpaOnnxOfflineRecognizerConfig" {
        feat_config: FeatureConfig,
        model_config: OfflineModelConfig,
        lm_config: OfflineLMConfig,
        decoding_method: Str,
        max_active_paths: i32,
        hotwords_file: Str,
        hotwords_score: f32,
        rule_fsts: Str,
        rule_fars: Str,
        blank_penalty: f32,
        hr: HomophoneReplacerConfig,
        ctc_fst_decoder_config: OfflineCtcFstDecoderConfig,
    }
    SileroVadModelConfig = "SherpaOnnxSileroVadModelConfig" {
        model: Str,
        threshold: f32,
        min_silence_duration: f32,
        min_speech_duration: f32,
        window_size: i32,
        max_speech_duration: f32,
    }
    TenVadModelConfig = "SherpaOnnxTenVadModelConfig" {
        model: Str,
        threshold: f32,
        min_silence_duration: f32,
        min_speech_duration: f32,
        window_size: i32,
        max_speech_duration: f32,
    }
    VadModelConfig = "SherpaOnnxVadModelConfig" {
        silero_vad: SileroVadModelConfig,
        sample_rate: i32,
        num_threads: i32,
        provider: Str,
        debug: i32,
        ten_vad: TenVadModelConfig,
    }
}

type Handle = *const c_void;

struct Fns {
    create_recognizer: unsafe extern "C" fn(*const OfflineRecognizerConfig) -> Handle,
    destroy_recognizer: unsafe extern "C" fn(Handle),
    create_stream: unsafe extern "C" fn(Handle) -> Handle,
    destroy_stream: unsafe extern "C" fn(Handle),
    accept_waveform: unsafe extern "C" fn(Handle, i32, *const f32, i32),
    decode: unsafe extern "C" fn(Handle, Handle),
    result_json: unsafe extern "C" fn(Handle) -> Str,
    destroy_json: unsafe extern "C" fn(Str),
    create_vad: unsafe extern "C" fn(*const VadModelConfig, f32) -> Handle,
    destroy_vad: unsafe extern "C" fn(Handle),
    vad_accept: unsafe extern "C" fn(Handle, *const f32, i32),
    vad_empty: unsafe extern "C" fn(Handle) -> i32,
    vad_detected: unsafe extern "C" fn(Handle) -> i32,
    vad_pop: unsafe extern "C" fn(Handle),
}

pub struct Api {
    f: Fns,
    _onnx: Library,
    _lib: Library,
}

#[cfg(unix)]
fn open(p: &Path) -> Result<Library, String> {
    use libloading::os::unix::{Library as L, RTLD_GLOBAL, RTLD_NOW};
    unsafe { L::open(Some(p), RTLD_NOW | RTLD_GLOBAL) }.map(Into::into).map_err(|e| e.to_string())
}

#[cfg(windows)]
fn open(p: &Path) -> Result<Library, String> {
    use libloading::os::windows::{Library as L, LOAD_WITH_ALTERED_SEARCH_PATH};
    unsafe { L::load_with_flags(p, LOAD_WITH_ALTERED_SEARCH_PATH) }.map(Into::into).map_err(|e| e.to_string())
}

macro_rules! sym {
    ($lib:expr, $name:literal) => {
        *unsafe { $lib.get(concat!($name, "\0").as_bytes()) }.map_err(|e| format!("{}: {e}", $name))?
    };
}

impl Api {
    /// Loads the runtime in DIR. Nothing but the version is called until the
    /// version is the pinned one.
    pub fn load(dir: &Path) -> Result<Arc<Api>, String> {
        let rt = local::runtime_dir(dir);
        // beside the C library, so it never comes from anywhere else
        let onnx = open(&rt.join(local::onnx_library()))?;
        let lib = open(&rt.join(local::c_library()))?;
        let version: unsafe extern "C" fn() -> Str = sym!(lib, "SherpaOnnxGetVersionStr");
        let got = unsafe {
            let v = version();
            if v.is_null() {
                String::new()
            } else {
                CStr::from_ptr(v).to_string_lossy().into_owned()
            }
        };
        if got != RUNTIME_VERSION {
            return Err(format!("the speech runtime is version {got}, this helper needs {RUNTIME_VERSION}; remove it and download it again (Settings → Voice)"));
        }
        let f = Fns {
            create_recognizer: sym!(lib, "SherpaOnnxCreateOfflineRecognizer"),
            destroy_recognizer: sym!(lib, "SherpaOnnxDestroyOfflineRecognizer"),
            create_stream: sym!(lib, "SherpaOnnxCreateOfflineStream"),
            destroy_stream: sym!(lib, "SherpaOnnxDestroyOfflineStream"),
            accept_waveform: sym!(lib, "SherpaOnnxAcceptWaveformOffline"),
            decode: sym!(lib, "SherpaOnnxDecodeOfflineStream"),
            result_json: sym!(lib, "SherpaOnnxGetOfflineStreamResultAsJson"),
            destroy_json: sym!(lib, "SherpaOnnxDestroyOfflineStreamResultJson"),
            create_vad: sym!(lib, "SherpaOnnxCreateVoiceActivityDetector"),
            destroy_vad: sym!(lib, "SherpaOnnxDestroyVoiceActivityDetector"),
            vad_accept: sym!(lib, "SherpaOnnxVoiceActivityDetectorAcceptWaveform"),
            vad_empty: sym!(lib, "SherpaOnnxVoiceActivityDetectorEmpty"),
            vad_detected: sym!(lib, "SherpaOnnxVoiceActivityDetectorDetected"),
            vad_pop: sym!(lib, "SherpaOnnxVoiceActivityDetectorPop"),
        };
        Ok(Arc::new(Api { f, _onnx: onnx, _lib: lib }))
    }
}

fn c(p: &Path) -> Result<CString, String> {
    CString::new(p.to_string_lossy().as_bytes()).map_err(|_| format!("a NUL in the path {}", p.display()))
}

fn lit(s: &str) -> CString {
    CString::new(s).unwrap()
}

/// The recognizer. Decoding runs on one thread at a time.
pub struct Recognizer {
    api: Arc<Api>,
    h: Handle,
}

// the handle is only ever used by whoever owns this
unsafe impl Send for Recognizer {}

impl Recognizer {
    pub fn create(api: &Arc<Api>, dir: &Path, id: &str, threads: i32) -> Result<Recognizer, String> {
        let m = local::model_dir(dir, id).ok_or_else(|| format!("unknown model {id}"))?;
        let (encoder, decoder, joiner) = (c(&m.join("encoder.int8.onnx"))?, c(&m.join("decoder.int8.onnx"))?, c(&m.join("joiner.int8.onnx"))?);
        let tokens = c(&m.join("tokens.txt"))?;
        let (provider, model_type, method) = (lit("cpu"), lit("nemo_transducer"), lit("greedy_search"));
        // every other field stays zero: the C API's defaults, as the addon left them
        let mut cfg: OfflineRecognizerConfig = unsafe { std::mem::zeroed() };
        cfg.feat_config.sample_rate = RATE as i32;
        cfg.feat_config.feature_dim = 80;
        cfg.model_config.transducer.encoder = encoder.as_ptr();
        cfg.model_config.transducer.decoder = decoder.as_ptr();
        cfg.model_config.transducer.joiner = joiner.as_ptr();
        cfg.model_config.tokens = tokens.as_ptr();
        cfg.model_config.num_threads = threads;
        cfg.model_config.provider = provider.as_ptr();
        cfg.model_config.model_type = model_type.as_ptr();
        cfg.decoding_method = method.as_ptr();
        let h = unsafe { (api.f.create_recognizer)(&cfg) };
        if h.is_null() {
            return Err("sherpa-onnx refused the model's configuration".into());
        }
        Ok(Recognizer { api: api.clone(), h })
    }

    /// One decode; the result is sherpa-onnx's JSON.
    pub fn decode_json(&self, samples: &[f32]) -> Result<String, String> {
        let f = &self.api.f;
        unsafe {
            let s = (f.create_stream)(self.h);
            if s.is_null() {
                return Err("sherpa-onnx gave no stream".into());
            }
            (f.accept_waveform)(s, RATE as i32, samples.as_ptr(), samples.len() as i32);
            (f.decode)(self.h, s);
            let j = (f.result_json)(s);
            let out = if j.is_null() {
                Err("sherpa-onnx gave no result".to_string())
            } else {
                let t = CStr::from_ptr(j).to_string_lossy().into_owned();
                (f.destroy_json)(j);
                Ok(t)
            };
            (f.destroy_stream)(s);
            out
        }
    }
}

impl Decoder for Recognizer {
    fn decode(&mut self, samples: &[f32]) -> Vec<Word> {
        let words = self.decode_json(samples).and_then(|j| serde_json::from_str(&j).map_err(|e| e.to_string()));
        match words {
            Ok(v) => local::words_of(&v),
            Err(e) => {
                crate::out::log(&format!("decode failed: {e}"));
                vec![]
            }
        }
    }
}

impl Drop for Recognizer {
    fn drop(&mut self) {
        unsafe { (self.api.f.destroy_recognizer)(self.h) }
    }
}

/// Silero VAD.
pub struct Vad {
    api: Arc<Api>,
    h: Handle,
}

unsafe impl Send for Vad {}

impl Vad {
    pub fn create(api: &Arc<Api>, dir: &Path) -> Result<Vad, String> {
        let model = c(&local::vad_path(dir))?;
        let mut cfg: VadModelConfig = unsafe { std::mem::zeroed() };
        cfg.silero_vad.model = model.as_ptr();
        cfg.silero_vad.threshold = 0.5;
        cfg.silero_vad.min_silence_duration = 0.5;
        cfg.silero_vad.min_speech_duration = MIN_SPEECH;
        cfg.silero_vad.window_size = 512;
        cfg.silero_vad.max_speech_duration = MAX_SEG;
        cfg.sample_rate = RATE as i32;
        cfg.num_threads = 1;
        let h = unsafe { (api.f.create_vad)(&cfg, 60.0) };
        if h.is_null() {
            return Err("sherpa-onnx refused the voice activity model".into());
        }
        Ok(Vad { api: api.clone(), h })
    }
}

impl Detector for Vad {
    fn accept(&mut self, w: &[f32]) {
        unsafe { (self.api.f.vad_accept)(self.h, w.as_ptr(), w.len() as i32) }
    }

    fn speaking(&mut self) -> bool {
        unsafe { (self.api.f.vad_detected)(self.h) != 0 }
    }

    /// Segments the VAD has closed since the last call (it keeps them queued).
    fn ended(&mut self) -> usize {
        let mut n = 0;
        unsafe {
            while (self.api.f.vad_empty)(self.h) == 0 {
                (self.api.f.vad_pop)(self.h);
                n += 1;
            }
        }
        n
    }
}

impl Drop for Vad {
    fn drop(&mut self) {
        unsafe { (self.api.f.destroy_vad)(self.h) }
    }
}

/// What a session needs: both built, ready to run; the recognizer is the
/// resident one at SERVER when it answers (the bool says so), else built here.
pub fn engine(dir: &Path, id: &str, server: &str) -> Result<(crate::resident::Rec, Vad, bool), String> {
    let api = Api::load(dir)?;
    crate::resident::turn_from(dir);
    let vad = Vad::create(&api, dir)?;
    let (rec, resident) = crate::resident::recognizer(&api, dir, id, server)?;
    Ok((rec, vad, resident))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt::Write as _;
    use std::process::Command;

    fn compiler() -> Option<&'static str> {
        ["cc", "clang", "gcc"].into_iter().find(|c| crate::util::which(c).is_some())
    }

    /// The C compiler reads the pinned header and prints what it thinks each
    /// struct looks like; every size and offset must match ours.
    #[test]
    fn the_structs_match_the_header() {
        let Some(cc) = compiler() else {
            eprintln!("no C compiler: layouts not checked");
            return;
        };
        let t = tempfile::tempdir().unwrap();
        let mut src = String::from("#include <stdio.h>\n#include <stddef.h>\n#include \"c-api.h\"\nint main(void) {\n");
        for (name, _, fields) in layouts() {
            writeln!(src, "printf(\"{name} %zu\\n\", sizeof({name}));").unwrap();
            for (f, _) in fields {
                writeln!(src, "printf(\"{name}.{f} %zu\\n\", offsetof({name}, {f}));").unwrap();
            }
        }
        src.push_str("return 0;\n}\n");
        let c = t.path().join("l.c");
        std::fs::write(&c, src).unwrap();
        let exe = t.path().join("l");
        let inc = concat!(env!("CARGO_MANIFEST_DIR"), "/sherpa");
        let o = Command::new(cc).arg("-w").arg("-I").arg(inc).arg(&c).arg("-o").arg(&exe).output().unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
        let out = String::from_utf8(Command::new(&exe).output().unwrap().stdout).unwrap();
        let mut want = String::new();
        for (name, size, fields) in layouts() {
            writeln!(want, "{name} {size}").unwrap();
            for (f, at) in fields {
                writeln!(want, "{name}.{f} {at}").unwrap();
            }
        }
        assert_eq!(out, want);
    }

    /// Every field the header gives a struct is in ours, by name and in order:
    /// the sizes alone would not see two swapped fields of one width.
    #[test]
    fn every_field_of_the_header_is_here() {
        let h = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/sherpa/c-api.h")).unwrap();
        for (name, _, fields) in layouts() {
            let start = h.find(&format!("typedef struct {name} {{")).unwrap_or_else(|| panic!("{name} is not in the header"));
            let end = start + h[start..].find(&format!("}} {name};")).unwrap();
            let mut got = vec![];
            let mut comment = false;
            for line in h[start..end].lines().skip(1) {
                let mut l = line.trim();
                if comment {
                    match l.find("*/") {
                        Some(i) => {
                            l = &l[i + 2..];
                            comment = false;
                        }
                        None => continue,
                    }
                }
                if l.starts_with("/*") {
                    comment = !l.contains("*/");
                    continue;
                }
                let l = l.split("//").next().unwrap().trim();
                if let Some(d) = l.strip_suffix(';') {
                    got.push(d.rsplit([' ', '*']).next().unwrap().to_string());
                }
            }
            let ours: Vec<String> = fields.iter().map(|f| f.0.to_string()).collect();
            assert_eq!(got, ours, "{name}");
        }
    }
}
