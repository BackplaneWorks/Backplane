//! Windows WASAPI audio capture and microphone enumeration.
//!
//! Format parsing, sample conversion/resampling, device list assembly, and
//! HRESULT error mapping live at module scope so they compile and are tested
//! on every platform; COM/WASAPI capture and enumeration are `#[cfg(windows)]`.

use crate::audio::{js_round, Resampler};
use crate::devices::{Device, Devices};

/// Sample representation produced by a WASAPI shared-mode stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SampleFormat {
    I16,
    I24,
    I32,
    F32,
}

impl SampleFormat {
    /// Bytes per channel sample for this format.
    pub fn bytes_per_sample(self) -> usize {
        match self {
            SampleFormat::I16 => 2,
            SampleFormat::I24 => 3,
            SampleFormat::I32 | SampleFormat::F32 => 4,
        }
    }
}

/// Decoded stream parameters of an open capture client.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CaptureFormat {
    pub sample_rate: u32,
    pub channels: u16,
    pub format: SampleFormat,
}

impl CaptureFormat {
    /// Bytes per interleaved audio frame across all channels.
    pub fn frame_bytes(self) -> usize {
        self.channels as usize * self.format.bytes_per_sample()
    }
}

/// Validates and classifies a WAVEFORMATEX / WAVEFORMATEXTENSIBLE descriptor.
pub fn describe_format_tag(
    format_tag: u16,
    sub_format_pcm: bool,
    sub_format_float: bool,
    bits_per_sample: u16,
    channels: u16,
    sample_rate: u32,
) -> Result<CaptureFormat, String> {
    if channels == 0 || sample_rate == 0 {
        return Err(format!("invalid capture format: {channels} ch @ {sample_rate} Hz"));
    }
    const WAVE_FORMAT_PCM_TAG: u16 = 1;
    const WAVE_FORMAT_IEEE_FLOAT_TAG: u16 = 3;
    const WAVE_FORMAT_EXTENSIBLE_TAG: u16 = 0xFFFE;

    let format = match format_tag {
        WAVE_FORMAT_PCM_TAG => match bits_per_sample {
            16 => SampleFormat::I16,
            24 => SampleFormat::I24,
            32 => SampleFormat::I32,
            other => return Err(format!("unsupported PCM bit depth: {other}")),
        },
        WAVE_FORMAT_IEEE_FLOAT_TAG => match bits_per_sample {
            32 => SampleFormat::F32,
            other => return Err(format!("unsupported IEEE float bit depth: {other}")),
        },
        WAVE_FORMAT_EXTENSIBLE_TAG => {
            if sub_format_float {
                match bits_per_sample {
                    32 => SampleFormat::F32,
                    other => return Err(format!("unsupported extensible float bit depth: {other}")),
                }
            } else if sub_format_pcm {
                match bits_per_sample {
                    16 => SampleFormat::I16,
                    24 => SampleFormat::I24,
                    32 => SampleFormat::I32,
                    other => return Err(format!("unsupported extensible PCM bit depth: {other}")),
                }
            } else {
                return Err("unsupported extensible audio subformat".to_string());
            }
        }
        other => return Err(format!("unsupported wave format tag: 0x{other:04x}")),
    };
    Ok(CaptureFormat { sample_rate, channels, format })
}

/// Converts interleaved capture frames to mono s16le PCM at `target_rate`.
pub struct PcmConverter {
    input: CaptureFormat,
    resampler: Option<Resampler>,
    carry: Vec<u8>,
}

impl PcmConverter {
    /// Creates a converter from `input` to mono s16le at `target_rate` Hz.
    pub fn new(input: CaptureFormat, target_rate: u32) -> Self {
        let resampler = (input.sample_rate != target_rate).then(|| Resampler::new(input.sample_rate as f64, target_rate as f64));
        Self { input, resampler, carry: Vec::new() }
    }

    /// Converts an interleaved capture packet into mono s16le PCM bytes at `target_rate`.
    pub fn push(&mut self, raw: &[u8]) -> Vec<u8> {
        if raw.is_empty() {
            return Vec::new();
        }
        if self.input.format == SampleFormat::I16
            && self.input.channels == 1
            && self.resampler.is_none()
            && self.carry.is_empty()
            && raw.len().is_multiple_of(2)
        {
            return raw.to_vec();
        }

        let frame_bytes = self.input.frame_bytes();
        let bps = self.input.format.bytes_per_sample();
        let channels = self.input.channels as usize;

        let owned;
        let data: &[u8] = if self.carry.is_empty() {
            raw
        } else {
            let mut merged = std::mem::take(&mut self.carry);
            merged.extend_from_slice(raw);
            owned = merged;
            &owned
        };

        let complete_len = (data.len() / frame_bytes) * frame_bytes;
        if complete_len < data.len() {
            self.carry.extend_from_slice(&data[complete_len..]);
        }
        if complete_len == 0 {
            return Vec::new();
        }

        let frames = complete_len / frame_bytes;
        let mut mono = Vec::with_capacity(frames * 2);
        for frame in data[..complete_len].chunks_exact(frame_bytes) {
            let mut sum = 0.0_f64;
            for ch in 0..channels {
                let off = ch * bps;
                let s = &frame[off..off + bps];
                let v = match self.input.format {
                    SampleFormat::I16 => i16::from_le_bytes([s[0], s[1]]) as f64 / 32768.0,
                    SampleFormat::I24 => {
                        let sign = if (s[2] & 0x80) != 0 { 0xFF } else { 0x00 };
                        let v = i32::from_le_bytes([s[0], s[1], s[2], sign]);
                        v as f64 / 8_388_608.0
                    }
                    SampleFormat::I32 => i32::from_le_bytes([s[0], s[1], s[2], s[3]]) as f64 / 2_147_483_648.0,
                    SampleFormat::F32 => {
                        let f = f32::from_le_bytes([s[0], s[1], s[2], s[3]]) as f64;
                        if f.is_finite() { f } else { 0.0 }
                    }
                };
                sum += v;
            }
            let avg = (sum / channels as f64).clamp(-1.0, 1.0);
            let out = js_round(avg * 32768.0).clamp(-32768.0, 32767.0) as i16;
            mono.extend_from_slice(&out.to_le_bytes());
        }

        match &mut self.resampler {
            Some(r) => r.push(&mono),
            None => mono,
        }
    }

    /// Emits `frames` input-rate frames of silence at `target_rate` so resampler phase stays continuous.
    pub fn silence(&mut self, frames: usize) -> Vec<u8> {
        if frames == 0 {
            return Vec::new();
        }
        let zeros = vec![0u8; frames * 2];
        match &mut self.resampler {
            Some(r) => r.push(&zeros),
            None => zeros,
        }
    }
}

/// Builds a `Devices` listing from `(id, friendly_name)` pairs and an optional default device ID.
pub fn resolve_devices(entries: Vec<(String, String)>, default_id: Option<String>) -> Devices {
    let devices = entries
        .into_iter()
        .filter(|(id, _)| !id.is_empty())
        .map(|(id, friendly)| {
            let label = if friendly.trim().is_empty() { id.clone() } else { friendly };
            Device { name: id, label }
        })
        .collect();
    Devices {
        default: default_id.unwrap_or_default(),
        devices,
        via: "wasapi".to_string(),
        error: None,
    }
}

/// Translates common Windows COM/WASAPI HRESULT codes into actionable messages.
pub fn friendly_hresult(code: u32, fallback: &str) -> String {
    match code {
        0x8007_0005 => "microphone access denied (check Windows Privacy -> Microphone settings)".to_string(),
        0x8007_0490 => "no microphone found".to_string(),
        0x8889_0004 => "microphone was disconnected".to_string(),
        0x8889_000A => "microphone is in use in exclusive mode by another app".to_string(),
        0x8889_0008 => "microphone audio format is not supported".to_string(),
        0x8889_0010 => "Windows Audio service is not running".to_string(),
        _ => format!("{fallback} (0x{code:08x})"),
    }
}

#[cfg(windows)]
mod imp {
    use super::*;
    use crate::recorder::Source;
    use std::ffi::c_void;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use tokio::sync::mpsc::error::TrySendError;
    use tokio::sync::{mpsc, oneshot};
    use windows::core::{Error as WinError, PCWSTR, PWSTR};
    use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
    use windows::Win32::Foundation::{CloseHandle, HANDLE, RPC_E_CHANGED_MODE, S_FALSE, S_OK, WAIT_FAILED, WAIT_OBJECT_0};
    use windows::Win32::Media::Audio::{
        eCapture, eCommunications, eConsole, IAudioCaptureClient, IAudioClient, IMMDevice, IMMDeviceEnumerator, MMDeviceEnumerator,
        AUDCLNT_BUFFERFLAGS_SILENT, AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM, AUDCLNT_STREAMFLAGS_EVENTCALLBACK,
        AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY, DEVICE_STATE_ACTIVE, WAVEFORMATEX, WAVEFORMATEXTENSIBLE, WAVE_FORMAT_PCM,
    };
    use windows::Win32::Media::KernelStreaming::{KSDATAFORMAT_SUBTYPE_PCM, WAVE_FORMAT_EXTENSIBLE};
    use windows::Win32::Media::Multimedia::KSDATAFORMAT_SUBTYPE_IEEE_FLOAT;
    use windows::Win32::System::Com::StructuredStorage::{PropVariantClear, PROPVARIANT};
    use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_ALL, COINIT_MULTITHREADED, STGM_READ};
    use windows::Win32::System::Threading::{CreateEventW, SetEvent, WaitForMultipleObjects, WaitForSingleObject};
    use windows::Win32::System::Variant::VT_LPWSTR;

    const CAPTURE_BUFFER_100NS: i64 = 1_000_000; // 100 ms in 100-ns units
    const WAIT_SLICE_MS: u32 = 200;

    fn hresult_msg(err: &WinError, fallback: &str) -> String {
        friendly_hresult(err.code().0 as u32, fallback)
    }

    struct ComGuard(bool);

    impl ComGuard {
        fn init() -> Result<Self, String> {
            let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
            if hr == S_OK || hr == S_FALSE {
                Ok(Self(true))
            } else if hr == RPC_E_CHANGED_MODE {
                Ok(Self(false))
            } else {
                Err(friendly_hresult(hr.0 as u32, "could not initialize Windows audio (COM)"))
            }
        }
    }

    impl Drop for ComGuard {
        fn drop(&mut self) {
            if self.0 {
                unsafe { CoUninitialize() };
            }
        }
    }

    struct CoTaskMemGuard<T>(*mut T);

    impl<T> Drop for CoTaskMemGuard<T> {
        fn drop(&mut self) {
            if !self.0.is_null() {
                unsafe { CoTaskMemFree(Some(self.0 as *const c_void)) };
            }
        }
    }

    struct PropVariantGuard(PROPVARIANT);

    impl Drop for PropVariantGuard {
        fn drop(&mut self) {
            unsafe {
                let _ = PropVariantClear(&mut self.0);
            }
        }
    }

    struct WinHandle(HANDLE);

    unsafe impl Send for WinHandle {}
    unsafe impl Sync for WinHandle {}

    impl WinHandle {
        fn new_event(manual_reset: bool) -> Result<Self, String> {
            let h = unsafe { CreateEventW(None, manual_reset, false, PCWSTR::null()) }
                .map_err(|e| hresult_msg(&e, "could not create audio event"))?;
            Ok(Self(h))
        }

        fn raw(&self) -> HANDLE {
            self.0
        }

        fn set(&self) {
            unsafe {
                let _ = SetEvent(self.0);
            }
        }
    }

    impl Drop for WinHandle {
        fn drop(&mut self) {
            if !self.0.is_invalid() {
                unsafe {
                    let _ = CloseHandle(self.0);
                }
            }
        }
    }

    fn take_pwstr(pw: PWSTR) -> String {
        if pw.is_null() {
            return String::new();
        }
        let _guard = CoTaskMemGuard(pw.as_ptr());
        unsafe { String::from_utf16_lossy(pw.as_wide()) }
    }

    fn device_id(dev: &IMMDevice) -> Option<String> {
        let pw = unsafe { dev.GetId() }.ok()?;
        let id = take_pwstr(pw);
        (!id.is_empty()).then_some(id)
    }

    fn device_friendly_name(dev: &IMMDevice) -> String {
        let Ok(store) = (unsafe { dev.OpenPropertyStore(STGM_READ) }) else {
            return String::new();
        };
        let Ok(raw) = (unsafe { store.GetValue(&PKEY_Device_FriendlyName) }) else {
            return String::new();
        };
        let guard = PropVariantGuard(raw);
        unsafe {
            let inner = &guard.0.Anonymous.Anonymous;
            if inner.vt == VT_LPWSTR {
                let pw = inner.Anonymous.pwszVal;
                if !pw.is_null() {
                    return String::from_utf16_lossy(pw.as_wide());
                }
            }
        }
        String::new()
    }

    fn enumerator() -> Result<IMMDeviceEnumerator, String> {
        unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }
            .map_err(|e| hresult_msg(&e, "could not open Windows audio device enumerator"))
    }

    fn default_capture_device(enumr: &IMMDeviceEnumerator) -> Result<IMMDevice, String> {
        unsafe { enumr.GetDefaultAudioEndpoint(eCapture, eConsole) }
            .or_else(|_| unsafe { enumr.GetDefaultAudioEndpoint(eCapture, eCommunications) })
            .map_err(|e| hresult_msg(&e, "no default microphone"))
    }

    fn resolve_capture_device(enumr: &IMMDeviceEnumerator, wanted: &str) -> Result<IMMDevice, String> {
        if wanted.is_empty() || wanted.eq_ignore_ascii_case("default") {
            return default_capture_device(enumr);
        }
        let wide: Vec<u16> = wanted.encode_utf16().chain(Some(0)).collect();
        if let Ok(dev) = unsafe { enumr.GetDevice(PCWSTR::from_raw(wide.as_ptr())) } {
            return Ok(dev);
        }
        let col = unsafe { enumr.EnumAudioEndpoints(eCapture, DEVICE_STATE_ACTIVE) }
            .map_err(|e| hresult_msg(&e, "could not list microphones"))?;
        let count = unsafe { col.GetCount() }.map_err(|e| hresult_msg(&e, "could not count microphones"))?;
        for i in 0..count {
            if let Ok(dev) = unsafe { col.Item(i) } {
                let label = device_friendly_name(&dev);
                if !label.is_empty() && label.eq_ignore_ascii_case(wanted) {
                    return Ok(dev);
                }
            }
        }
        Err(format!("microphone not found: {wanted}"))
    }

    fn parse_wave_format(wf_ptr: *const WAVEFORMATEX) -> Result<CaptureFormat, String> {
        let wf = unsafe { *wf_ptr };
        let (sub_pcm, sub_float) = if wf.wFormatTag as u32 == WAVE_FORMAT_EXTENSIBLE {
            let ext = unsafe { *(wf_ptr as *const WAVEFORMATEXTENSIBLE) };
            let sub = ext.SubFormat;
            (sub == KSDATAFORMAT_SUBTYPE_PCM, sub == KSDATAFORMAT_SUBTYPE_IEEE_FLOAT)
        } else {
            (false, false)
        };
        let fmt = describe_format_tag(wf.wFormatTag, sub_pcm, sub_float, wf.wBitsPerSample, wf.nChannels, wf.nSamplesPerSec)?;
        let block_align = wf.nBlockAlign as usize;
        if block_align != fmt.frame_bytes() {
            return Err(format!("unexpected audio block align {block_align} (expected {})", fmt.frame_bytes()));
        }
        Ok(fmt)
    }

    fn init_capture_client(dev: &IMMDevice, rate: u32) -> Result<(IAudioClient, CaptureFormat), String> {
        let desired = WAVEFORMATEX {
            wFormatTag: WAVE_FORMAT_PCM as u16,
            nChannels: 1,
            nSamplesPerSec: rate,
            nAvgBytesPerSec: rate * 2,
            nBlockAlign: 2,
            wBitsPerSample: 16,
            cbSize: 0,
        };
        let client: IAudioClient = unsafe { dev.Activate(CLSCTX_ALL, None) }
            .map_err(|e| hresult_msg(&e, "could not open microphone"))?;
        let auto_flags = AUDCLNT_STREAMFLAGS_EVENTCALLBACK | AUDCLNT_STREAMFLAGS_AUTOCONVERTPCM | AUDCLNT_STREAMFLAGS_SRC_DEFAULT_QUALITY;
        if let Err(e) = unsafe { client.Initialize(AUDCLNT_SHAREMODE_SHARED, auto_flags, CAPTURE_BUFFER_100NS, 0, &desired, None) } {
            crate::out::log(&format!(
                "wasapi: autoconvert init failed ({}); falling back to mix format",
                hresult_msg(&e, "Initialize")
            ));
        } else {
            return Ok((client, CaptureFormat { sample_rate: rate, channels: 1, format: SampleFormat::I16 }));
        }
        // Some drivers reject AUTOCONVERTPCM or require a fresh IAudioClient after a failed Initialize.
        let client: IAudioClient = unsafe { dev.Activate(CLSCTX_ALL, None) }
            .map_err(|e| hresult_msg(&e, "could not open microphone"))?;
        let mix_ptr = unsafe { client.GetMixFormat() }
            .map_err(|e| hresult_msg(&e, "could not read microphone mix format"))?;
        let _mix_guard = CoTaskMemGuard(mix_ptr);
        let fmt = parse_wave_format(mix_ptr)?;
        unsafe { client.Initialize(AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_EVENTCALLBACK, CAPTURE_BUFFER_100NS, 0, mix_ptr, None) }
            .map_err(|e| hresult_msg(&e, "could not initialize microphone capture"))?;
        Ok((client, fmt))
    }

    fn send_packet(
        mut pcm: Vec<u8>,
        audio_tx: &mpsc::Sender<Vec<u8>>,
        stop_flag: &AtomicBool,
        stop_event: &WinHandle,
    ) -> bool {
        loop {
            match audio_tx.try_send(pcm) {
                Ok(()) => return true,
                Err(TrySendError::Closed(_)) => return false,
                Err(TrySendError::Full(returned)) => {
                    if stop_flag.load(Ordering::SeqCst) {
                        return false;
                    }
                    let wait = unsafe { WaitForSingleObject(stop_event.raw(), 10) };
                    if wait == WAIT_OBJECT_0 {
                        return false;
                    }
                    pcm = returned;
                }
            }
        }
    }

    fn drain_packets(
        capture: &IAudioCaptureClient,
        converter: &mut PcmConverter,
        audio_tx: &mpsc::Sender<Vec<u8>>,
        frame_bytes: usize,
        stop_flag: &AtomicBool,
        stop_event: &WinHandle,
    ) -> Result<bool, String> {
        loop {
            let packet_frames = unsafe { capture.GetNextPacketSize() }
                .map_err(|e| hresult_msg(&e, "audio capture stopped"))?;
            if packet_frames == 0 {
                return Ok(true);
            }
            let mut data_ptr = std::ptr::null_mut();
            let mut num_frames = 0u32;
            let mut flags = 0u32;
            unsafe { capture.GetBuffer(&mut data_ptr, &mut num_frames, &mut flags, None, None) }
                .map_err(|e| hresult_msg(&e, "audio capture stopped"))?;
            let pcm = if num_frames == 0 {
                Vec::new()
            } else if (flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32) != 0 || data_ptr.is_null() {
                converter.silence(num_frames as usize)
            } else {
                let len = num_frames as usize * frame_bytes;
                let slice = unsafe { std::slice::from_raw_parts(data_ptr, len) };
                converter.push(slice)
            };
            let rel = unsafe { capture.ReleaseBuffer(num_frames) };
            if let Err(e) = rel {
                return Err(hresult_msg(&e, "audio capture stopped"));
            }
            if !pcm.is_empty() && !send_packet(pcm, audio_tx, stop_flag, stop_event) {
                return Ok(false);
            }
        }
    }

    pub async fn list_devices() -> Devices {
        tokio::task::spawn_blocking(|| {
            let _com = match ComGuard::init() {
                Ok(c) => c,
                Err(e) => return Devices { error: Some(e), ..Default::default() },
            };
            let enumr = match enumerator() {
                Ok(e) => e,
                Err(e) => return Devices { error: Some(e), ..Default::default() },
            };
            let default_id = default_capture_device(&enumr).ok().and_then(|d| device_id(&d));
            let col = match unsafe { enumr.EnumAudioEndpoints(eCapture, DEVICE_STATE_ACTIVE) } {
                Ok(c) => c,
                Err(e) => return Devices { error: Some(hresult_msg(&e, "could not list microphones")), ..Default::default() },
            };
            let count = unsafe { col.GetCount() }.unwrap_or(0);
            let mut entries = Vec::with_capacity(count as usize);
            for i in 0..count {
                if let Ok(dev) = unsafe { col.Item(i) } {
                    if let Some(id) = device_id(&dev) {
                        let label = device_friendly_name(&dev);
                        entries.push((id, label));
                    }
                }
            }
            resolve_devices(entries, default_id)
        })
        .await
        .unwrap_or_else(|e| Devices { error: Some(format!("could not list microphones: {e}")), ..Default::default() })
    }

    pub async fn open(device: &str, rate: u32) -> Result<Source, String> {
        let stop_flag = Arc::new(AtomicBool::new(false));
        let stop_event = Arc::new(WinHandle::new_event(true)?);
        let (audio_tx, audio_rx) = mpsc::channel(256);
        let (ready_tx, ready_rx) = oneshot::channel::<Result<(), String>>();
        let (why_tx, why_rx) = oneshot::channel::<String>();

        let device_owned = device.to_string();
        let stop_flag_t = stop_flag.clone();
        let stop_event_t = stop_event.clone();

        std::thread::Builder::new()
            .name("wasapi-capture".to_string())
            .spawn(move || {
                // `_com` is declared first so it drops after all COM interfaces.
                let _com = match ComGuard::init() {
                    Ok(c) => c,
                    Err(e) => {
                        let _ = ready_tx.send(Err(e));
                        return;
                    }
                };
                let setup = (|| -> Result<(IAudioClient, IAudioCaptureClient, WinHandle, CaptureFormat), String> {
                    let enumr = enumerator()?;
                    let imm = resolve_capture_device(&enumr, &device_owned)?;
                    let (client, fmt) = init_capture_client(&imm, rate)?;
                    let audio_event = WinHandle::new_event(false)?;
                    unsafe { client.SetEventHandle(audio_event.raw()) }
                        .map_err(|e| hresult_msg(&e, "could not attach audio capture event"))?;
                    let capture: IAudioCaptureClient = unsafe { client.GetService() }
                        .map_err(|e| hresult_msg(&e, "could not create audio capture client"))?;
                    unsafe { client.Start() }
                        .map_err(|e| hresult_msg(&e, "could not start microphone capture"))?;
                    Ok((client, capture, audio_event, fmt))
                })();

                let (client, capture, audio_event, fmt) = match setup {
                    Ok(v) => {
                        let _ = ready_tx.send(Ok(()));
                        v
                    }
                    Err(e) => {
                        let _ = ready_tx.send(Err(e));
                        return;
                    }
                };

                let mut converter = PcmConverter::new(fmt, rate);
                let frame_bytes = fmt.frame_bytes();
                let handles = [stop_event_t.raw(), audio_event.raw()];
                let mut stop_reason = String::new();

                while !stop_flag_t.load(Ordering::SeqCst) {
                    match drain_packets(&capture, &mut converter, &audio_tx, frame_bytes, &stop_flag_t, &stop_event_t) {
                        Ok(true) => {}
                        Ok(false) => break,
                        Err(e) => {
                            stop_reason = e;
                            break;
                        }
                    }
                    let wait = unsafe { WaitForMultipleObjects(&handles, false, WAIT_SLICE_MS) };
                    if wait == WAIT_OBJECT_0 {
                        break;
                    }
                    if wait == WAIT_FAILED {
                        stop_reason = "audio capture wait failed".to_string();
                        break;
                    }
                }

                unsafe {
                    let _ = client.Stop();
                }
                if stop_reason.is_empty() {
                    let _ = drain_packets(&capture, &mut converter, &audio_tx, frame_bytes, &stop_flag_t, &stop_event_t);
                } else {
                    let _ = why_tx.send(stop_reason);
                }
                drop(audio_tx);
                drop(capture);
                drop(client);
            })
            .map_err(|e| format!("could not start capture thread: {e}"))?;

        match ready_rx.await {
            Ok(Ok(())) => Ok(Source::native(
                "wasapi",
                audio_rx,
                move || {
                    stop_flag.store(true, Ordering::SeqCst);
                    stop_event.set();
                },
                why_rx,
            )),
            Ok(Err(e)) => Err(e),
            Err(_) => Err("capture thread exited unexpectedly".to_string()),
        }
    }
}

#[cfg(windows)]
pub use imp::{list_devices, open};

#[cfg(test)]
mod tests {
    use super::*;

    fn i16s(b: &[u8]) -> Vec<i16> {
        b.chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]])).collect()
    }

    #[test]
    fn pcm_converter_passthrough_mono_i16_same_rate() {
        let mut c = PcmConverter::new(CaptureFormat { sample_rate: 16000, channels: 1, format: SampleFormat::I16 }, 16000);
        let raw = [0x01u8, 0x02, 0xFE, 0xFF];
        assert_eq!(c.push(&raw), raw);
        // Odd byte splits across packets cleanly.
        assert_eq!(c.push(&[0x34]), Vec::<u8>::new());
        assert_eq!(c.push(&[0x12]), vec![0x34, 0x12]);
    }

    #[test]
    fn pcm_converter_stereo_f32_downmix_clamps_and_handles_nan() {
        let mut c = PcmConverter::new(CaptureFormat { sample_rate: 16000, channels: 2, format: SampleFormat::F32 }, 16000);
        let mut raw = Vec::new();
        for (l, r) in [(0.5_f32, -0.5_f32), (2.0_f32, 2.0_f32), (-2.0_f32, -2.0_f32), (f32::NAN, 0.5_f32)] {
            raw.extend_from_slice(&l.to_le_bytes());
            raw.extend_from_slice(&r.to_le_bytes());
        }
        let out = i16s(&c.push(&raw));
        assert_eq!(out, vec![0, 32767, -32768, 8192]);
    }

    #[test]
    fn pcm_converter_i24_and_i32_to_mono_i16() {
        let mut c24 = PcmConverter::new(CaptureFormat { sample_rate: 16000, channels: 1, format: SampleFormat::I24 }, 16000);
        let raw24 = [0x00, 0x00, 0x40, 0x00, 0x00, 0xC0]; // +0.5, -0.5 in 24-bit
        assert_eq!(i16s(&c24.push(&raw24)), vec![16384, -16384]);

        let mut c32 = PcmConverter::new(CaptureFormat { sample_rate: 16000, channels: 1, format: SampleFormat::I32 }, 16000);
        let mut raw32 = Vec::new();
        raw32.extend_from_slice(&(i32::MAX / 2).to_le_bytes());
        raw32.extend_from_slice(&(i32::MIN / 2).to_le_bytes());
        let out32 = i16s(&c32.push(&raw32));
        assert!((out32[0] - 16384).abs() <= 1);
        assert!((out32[1] + 16384).abs() <= 1);
    }

    #[test]
    fn pcm_converter_resamples_48khz_stereo_f32_to_16khz_mono_i16() {
        let mut c = PcmConverter::new(CaptureFormat { sample_rate: 48000, channels: 2, format: SampleFormat::F32 }, 16000);
        let mut raw = Vec::new();
        for i in 0..4800 {
            let s = (i as f32 / 30.0).sin() * 0.25;
            raw.extend_from_slice(&s.to_le_bytes());
            raw.extend_from_slice(&s.to_le_bytes());
        }
        let out = i16s(&c.push(&raw));
        assert!((out.len() as i32 - 1600).abs() <= 2, "got {} samples", out.len());
    }

    #[test]
    fn pcm_converter_silence_same_rate_and_resampled() {
        let mut same = PcmConverter::new(CaptureFormat { sample_rate: 16000, channels: 2, format: SampleFormat::F32 }, 16000);
        assert_eq!(same.silence(4), vec![0u8; 8]);
        assert!(same.silence(0).is_empty());

        let mut resamp = PcmConverter::new(CaptureFormat { sample_rate: 48000, channels: 2, format: SampleFormat::F32 }, 16000);
        let s = i16s(&resamp.silence(300));
        assert!((s.len() as i32 - 100).abs() <= 1);
        assert!(s.iter().all(|&x| x == 0));
    }

    #[test]
    fn describe_format_tag_parses_pcm_float_extensible_and_rejects_invalid() {
        assert_eq!(
            describe_format_tag(1, false, false, 16, 1, 16000).unwrap(),
            CaptureFormat { sample_rate: 16000, channels: 1, format: SampleFormat::I16 }
        );
        assert_eq!(
            describe_format_tag(3, false, false, 32, 2, 48000).unwrap(),
            CaptureFormat { sample_rate: 48000, channels: 2, format: SampleFormat::F32 }
        );
        assert_eq!(
            describe_format_tag(0xFFFE, true, false, 24, 2, 44100).unwrap(),
            CaptureFormat { sample_rate: 44100, channels: 2, format: SampleFormat::I24 }
        );
        assert_eq!(
            describe_format_tag(0xFFFE, true, false, 32, 2, 48000).unwrap(),
            CaptureFormat { sample_rate: 48000, channels: 2, format: SampleFormat::I32 }
        );
        assert_eq!(
            describe_format_tag(0xFFFE, false, true, 32, 2, 48000).unwrap(),
            CaptureFormat { sample_rate: 48000, channels: 2, format: SampleFormat::F32 }
        );
        assert!(describe_format_tag(1, false, false, 16, 0, 16000).is_err());
        assert!(describe_format_tag(1, false, false, 8, 1, 16000).is_err());
        assert!(describe_format_tag(0xFFFE, false, false, 32, 2, 48000).is_err());
        assert!(describe_format_tag(0xFFFE, false, true, 64, 2, 48000).is_err());
        assert!(describe_format_tag(0xFFFE, true, false, 8, 2, 48000).is_err());
        assert!(describe_format_tag(2, false, false, 16, 1, 16000).is_err());
    }

    #[test]
    fn resolve_devices_builds_wasapi_list_and_defaults() {
        let d = resolve_devices(
            vec![
                ("{0.0.1}.{mic1}".into(), "USB Condenser".into()),
                ("".into(), "Skipped".into()),
                ("{0.0.1}.{mic2}".into(), "  ".into()),
            ],
            Some("{0.0.1}.{mic1}".into()),
        );
        assert_eq!(d.via, "wasapi");
        assert_eq!(d.default, "{0.0.1}.{mic1}");
        assert_eq!(d.error, None);
        assert_eq!(
            d.devices,
            vec![
                Device { name: "{0.0.1}.{mic1}".into(), label: "USB Condenser".into() },
                Device { name: "{0.0.1}.{mic2}".into(), label: "{0.0.1}.{mic2}".into() },
            ]
        );
    }

    #[test]
    fn friendly_hresult_maps_common_wasapi_errors() {
        assert!(friendly_hresult(0x8007_0005, "open").contains("Privacy"));
        assert_eq!(friendly_hresult(0x8007_0490, "open"), "no microphone found");
        assert!(friendly_hresult(0x8889_0004, "open").contains("disconnected"));
        assert!(friendly_hresult(0x8889_000A, "open").contains("exclusive mode"));
        assert!(friendly_hresult(0x8889_0008, "open").contains("not supported"));
        assert!(friendly_hresult(0x8889_0010, "open").contains("Windows Audio service"));
        assert_eq!(friendly_hresult(0x8000_4005, "failed"), "failed (0x80004005)");
    }
}
