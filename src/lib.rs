//! Opus voice codec for Godot 4: `OpusEncoder` and `OpusDecoder`.
//!
//! ```gdscript
//! var encoder := OpusEncoder.new()
//! encoder.setup(16000, 1, 24000, true)        # rate, channels, bitrate, voice
//! var packet := encoder.encode(frame)         # get_frame_size() samples per channel
//! var decoder := OpusDecoder.new()
//! decoder.setup(16000, 1)
//! var pcm := decoder.decode(packet)           # or decode_lost() for a missing packet
//! ```

use godot::prelude::*;

pub mod codec;

use codec::{Format, VoiceDecoder, VoiceEncoder};

struct GodotOpus;

#[gdextension]
unsafe impl ExtensionLibrary for GodotOpus {}

/// Encodes 20 ms frames of PCM (floats in [-1, 1], interleaved when
/// stereo) into Opus packets.
#[derive(GodotClass)]
#[class(base=RefCounted, init)]
pub struct OpusEncoder {
    base: Base<RefCounted>,
    inner: Option<VoiceEncoder>,
}

#[godot_api]
impl OpusEncoder {
    /// `sample_rate`: 8000, 12000, 16000, 24000 or 48000. `channels`: 1 or 2.
    /// `bitrate` in bits per second. `voice` tunes for speech (VoIP),
    /// otherwise for music. Returns false (and logs why) on bad settings.
    #[func]
    fn setup(&mut self, sample_rate: i32, channels: i32, bitrate: i32, voice: bool) -> bool {
        let result = Format::new(sample_rate.max(0) as u32, channels.max(0) as u32)
            .and_then(|format| VoiceEncoder::new(format, bitrate, voice));
        match result {
            Ok(encoder) => {
                self.inner = Some(encoder);
                true
            }
            Err(err) => {
                godot_error!("OpusEncoder.setup: {err}");
                false
            }
        }
    }

    /// Samples per channel in one frame (20 ms).
    #[func]
    fn get_frame_size(&self) -> i32 {
        self.inner
            .as_ref()
            .map_or(0, |e| e.format.frame_size() as i32)
    }

    /// Encodes exactly one frame (`get_frame_size() * channels` samples).
    /// Returns an empty array on error.
    #[func]
    fn encode(&mut self, pcm: PackedFloat32Array) -> PackedByteArray {
        let Some(encoder) = self.inner.as_mut() else {
            godot_error!("OpusEncoder.encode: call setup() first");
            return PackedByteArray::new();
        };
        match encoder.encode(pcm.as_slice()) {
            Ok(packet) => PackedByteArray::from(packet.as_slice()),
            Err(err) => {
                godot_error!("OpusEncoder.encode: {err}");
                PackedByteArray::new()
            }
        }
    }
}

/// Decodes Opus packets back to PCM (floats, interleaved when stereo).
#[derive(GodotClass)]
#[class(base=RefCounted, init)]
pub struct OpusDecoder {
    base: Base<RefCounted>,
    inner: Option<VoiceDecoder>,
}

#[godot_api]
impl OpusDecoder {
    /// Same sample rates and channel counts as `OpusEncoder.setup`; they
    /// don't have to match the encoder's (Opus resamples).
    #[func]
    fn setup(&mut self, sample_rate: i32, channels: i32) -> bool {
        let result = Format::new(sample_rate.max(0) as u32, channels.max(0) as u32)
            .and_then(VoiceDecoder::new);
        match result {
            Ok(decoder) => {
                self.inner = Some(decoder);
                true
            }
            Err(err) => {
                godot_error!("OpusDecoder.setup: {err}");
                false
            }
        }
    }

    /// Samples per channel in one frame (20 ms).
    #[func]
    fn get_frame_size(&self) -> i32 {
        self.inner
            .as_ref()
            .map_or(0, |d| d.format.frame_size() as i32)
    }

    /// Decodes one packet. Returns an empty array for a corrupt packet.
    #[func]
    fn decode(&mut self, packet: PackedByteArray) -> PackedFloat32Array {
        self.run(|decoder| decoder.decode(packet.as_slice()))
    }

    /// One frame standing in for a lost packet. Pass the packet that came
    /// after the lost one to rebuild it from its error-correction data;
    /// pass an empty array to synthesize it (packet-loss concealment).
    #[func]
    fn decode_lost(&mut self, next_packet: PackedByteArray) -> PackedFloat32Array {
        let next = (!next_packet.is_empty()).then(|| next_packet.as_slice().to_vec());
        self.run(|decoder| decoder.conceal(next.as_deref()))
    }
}

impl OpusDecoder {
    fn run(
        &mut self,
        f: impl FnOnce(&mut VoiceDecoder) -> Result<Vec<f32>, String>,
    ) -> PackedFloat32Array {
        let Some(decoder) = self.inner.as_mut() else {
            godot_error!("OpusDecoder: call setup() first");
            return PackedFloat32Array::new();
        };
        match f(decoder) {
            Ok(pcm) => PackedFloat32Array::from(pcm.as_slice()),
            Err(err) => {
                godot_error!("OpusDecoder: {err}");
                PackedFloat32Array::new()
            }
        }
    }
}
