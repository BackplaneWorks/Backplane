# say.ps1 OUT.wav "words": speech from Windows' own synthesizer as a 16 kHz mono 16-bit WAV (the recording test/e2e/voice-effect.mjs dictates)
param([string]$Out, [string]$Text = 'Please open the schematic and check the regulator before lunch.')
Add-Type -AssemblyName System.Speech
$s = New-Object System.Speech.Synthesis.SpeechSynthesizer
$f = New-Object System.Speech.AudioFormat.SpeechAudioFormatInfo(16000, [System.Speech.AudioFormat.AudioBitsPerSample]::Sixteen, [System.Speech.AudioFormat.AudioChannel]::Mono)
$s.SetOutputToWaveFile($Out, $f); $s.Speak($Text); $s.Dispose()
"wrote $Out ($((Get-Item $Out).Length) bytes)"
