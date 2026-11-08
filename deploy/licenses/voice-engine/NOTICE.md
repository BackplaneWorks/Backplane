# Dictation's local engine (downloaded, not shipped)

None of these files is in this archive. When you turn on the local engine in
Settings → Voice, the voice helper (`backplane-voice`) downloads them into the
hub's voice folder, checks each against a pinned size and sha256
(native/voice/src/local.rs), and loads them from there. Their notices are here
so that you have them with the program that fetches them.

| What the helper downloads | Version | Licence | Text here |
|---|---|---|---|
| sherpa-onnx's runtime: `libsherpa-onnx-c-api` and its Node addon, from the npm package `sherpa-onnx-<platform>` (https://github.com/k2-fsa/sherpa-onnx) | 2.0.1 | Apache-2.0 | `sherpa-onnx-LICENSE` |
| ONNX Runtime (`libonnxruntime`), inside that package (https://github.com/microsoft/onnxruntime) | 1.28.2 | MIT | `onnxruntime-LICENSE` |
| NVIDIA Parakeet TDT 0.6B v2 or v3 (int8), the sherpa-onnx exports `sherpa-onnx-nemo-parakeet-tdt-0.6b-v2-int8` and `-v3-int8` (https://huggingface.co/nvidia/parakeet-tdt-0.6b-v2, https://huggingface.co/nvidia/parakeet-tdt-0.6b-v3) | v2, v3 | CC-BY-4.0 | `CC-BY-4.0.txt` |
| Silero VAD (`silero_vad.onnx`), only when the copy beside the helper is missing | v4.0 | MIT | `../silero-vad/LICENSE` |

Attribution for the models: "Parakeet TDT 0.6B" by NVIDIA, licensed under
CC BY 4.0, converted to ONNX and quantised to int8 by the sherpa-onnx project.
Backplane does not change them.
