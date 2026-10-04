# godot-opus

Opus voice codec for Godot 4 as a Rust GDExtension: `OpusEncoder` and
`OpusDecoder`. libopus is compiled from source and linked statically, so
the library has no system dependencies. Made for voice chat (Smoking
Snakes' team radio), it works for any audio.

```gdscript
var encoder := OpusEncoder.new()
encoder.setup(16000, 1, 24000, true)   # sample rate, channels, bitrate, voice
var frame_size := encoder.get_frame_size()   # samples per channel in 20 ms
var packet := encoder.encode(pcm)      # PackedFloat32Array, one frame

var decoder := OpusDecoder.new()
decoder.setup(16000, 1)
var out := decoder.decode(packet)      # PackedFloat32Array
var filler := decoder.decode_lost(next_packet)   # a lost packet: FEC or concealment
```

| Class | Method | Notes |
| --- | --- | --- |
| `OpusEncoder` | `setup(sample_rate, channels, bitrate, voice) -> bool` | Rates 8000, 12000, 16000, 24000 or 48000; 1 or 2 channels; `voice` = VoIP mode, otherwise music. Forward error correction is on. |
| | `get_frame_size() -> int` | Samples per channel per 20 ms frame. |
| | `encode(pcm: PackedFloat32Array) -> PackedByteArray` | Exactly one frame, interleaved when stereo; one packet out. |
| `OpusDecoder` | `setup(sample_rate, channels) -> bool` | Needn't match the encoder: Opus resamples. |
| | `decode(packet) -> PackedFloat32Array` | Empty on a corrupt packet. |
| | `decode_lost(next_packet) -> PackedFloat32Array` | One frame for a lost packet: rebuilt from `next_packet`'s FEC data, or concealed when it is empty. |

Errors (bad settings, wrong frame size) are logged with `push_error` and
return `false` or an empty array. Godot's own mix rate is usually 44.1 or
48 kHz: resample microphone audio to one of the Opus rates first.

## Install

Add the library for each platform under `res://libs/` and a
`godot_opus.gdextension`:

```ini
[configuration]
entry_symbol = "gdext_rust_init"
compatibility_minimum = 4.5

[libraries]
linux.debug.x86_64 = "res://libs/libgodot_opus.so"
linux.release.x86_64 = "res://libs/libgodot_opus.so"
windows.debug.x86_64 = "res://libs/godot_opus.dll"
windows.release.x86_64 = "res://libs/godot_opus.dll"
macos.debug = "res://libs/libgodot_opus.dylib"
macos.release = "res://libs/libgodot_opus.dylib"
```

CI builds all three (the macOS one is universal) as artifacts.

## Build and test

```sh
cargo build --release          # needs cmake and a C compiler (libopus)
cargo test                     # the codec, without Godot
GODOT=/path/to/godot tests/run.sh   # the Godot classes, headless
```

`godot_opus` loads alongside godot-whisper: its classes are named
`OpusEncoder` and `OpusDecoder`, not `OpusEncoderNode` and
`OpusDecoderNode`.
