//! Plain-Rust Opus voice codec, wrapped by the Godot classes in `lib.rs`
//! (kept separate so it can be tested without the engine).

use opus2::{Application, Bitrate, Channels, Decoder, Encoder, Signal};

/// Sample rates Opus accepts.
pub const RATES: [u32; 5] = [8000, 12000, 16000, 24000, 48000];
/// Largest possible Opus packet (RFC 6716).
const MAX_PACKET: usize = 1275 * 3 + 7;
/// Frames are 20 ms.
const FRAMES_PER_SECOND: u32 = 50;

fn channels(count: u32) -> Option<Channels> {
    match count {
        1 => Some(Channels::Mono),
        2 => Some(Channels::Stereo),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Format {
    pub sample_rate: u32,
    pub channels: u32,
}

impl Format {
    pub fn new(sample_rate: u32, channels: u32) -> Result<Self, String> {
        if !RATES.contains(&sample_rate) {
            return Err(format!("sample rate {sample_rate} is not one of {RATES:?}"));
        }
        if !(1..=2).contains(&channels) {
            return Err(format!("{channels} channels: use 1 or 2"));
        }
        Ok(Self {
            sample_rate,
            channels,
        })
    }

    /// Samples per channel in one 20 ms frame.
    pub fn frame_size(&self) -> usize {
        (self.sample_rate / FRAMES_PER_SECOND) as usize
    }

    /// Interleaved samples in one frame.
    pub fn frame_len(&self) -> usize {
        self.frame_size() * self.channels as usize
    }
}

pub struct VoiceEncoder {
    encoder: Encoder,
    pub format: Format,
}

impl VoiceEncoder {
    /// `voice` tunes Opus for speech (VoIP mode); otherwise for music.
    pub fn new(format: Format, bitrate: i32, voice: bool) -> Result<Self, String> {
        let application = if voice {
            Application::Voip
        } else {
            Application::Audio
        };
        let mut encoder = Encoder::new(
            format.sample_rate,
            channels(format.channels).unwrap(),
            application,
        )
        .map_err(|err| err.to_string())?;
        encoder
            .set_bitrate(Bitrate::Bits(bitrate.clamp(6000, 510_000)))
            .map_err(|err| err.to_string())?;
        encoder
            .set_signal(if voice { Signal::Voice } else { Signal::Music })
            .map_err(|err| err.to_string())?;
        // In-band forward error correction lets the decoder rebuild a lost
        // frame from the next one.
        encoder
            .set_inband_fec(true)
            .map_err(|err| err.to_string())?;
        encoder
            .set_packet_loss_perc(10)
            .map_err(|err| err.to_string())?;
        Ok(Self { encoder, format })
    }

    /// Encodes one 20 ms frame of interleaved samples into one packet.
    pub fn encode(&mut self, pcm: &[f32]) -> Result<Vec<u8>, String> {
        if pcm.len() != self.format.frame_len() {
            return Err(format!(
                "expected {} samples, got {}",
                self.format.frame_len(),
                pcm.len()
            ));
        }
        let mut packet = vec![0u8; MAX_PACKET];
        let len = self
            .encoder
            .encode_float(pcm, &mut packet)
            .map_err(|err| err.to_string())?;
        packet.truncate(len);
        Ok(packet)
    }
}

pub struct VoiceDecoder {
    decoder: Decoder,
    pub format: Format,
}

impl VoiceDecoder {
    pub fn new(format: Format) -> Result<Self, String> {
        let decoder = Decoder::new(format.sample_rate, channels(format.channels).unwrap())
            .map_err(|err| err.to_string())?;
        Ok(Self { decoder, format })
    }

    /// Decodes one packet into interleaved samples.
    pub fn decode(&mut self, packet: &[u8]) -> Result<Vec<f32>, String> {
        self.run(packet, false)
    }

    /// Rebuilds a lost frame: from the forward error correction carried by
    /// `next` (the packet after the lost one) when given, otherwise by
    /// packet-loss concealment.
    pub fn conceal(&mut self, next: Option<&[u8]>) -> Result<Vec<f32>, String> {
        match next {
            Some(next) => self.run(next, true),
            None => self.run(&[], false),
        }
    }

    fn run(&mut self, packet: &[u8], fec: bool) -> Result<Vec<f32>, String> {
        // Up to 120 ms per packet; FEC and concealment produce one frame.
        let samples = if fec || packet.is_empty() {
            self.format.frame_size()
        } else {
            self.format.frame_size() * 6
        };
        let mut pcm = vec![0f32; samples * self.format.channels as usize];
        let decoded = self
            .decoder
            .decode_float(packet, &mut pcm, fec)
            .map_err(|err| err.to_string())?;
        pcm.truncate(decoded * self.format.channels as usize);
        Ok(pcm)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(format: Format, frames: usize) -> Vec<Vec<f32>> {
        let mut t = 0usize;
        (0..frames)
            .map(|_| {
                (0..format.frame_size())
                    .flat_map(|_| {
                        let s =
                            (t as f32 / format.sample_rate as f32 * 440.0 * std::f32::consts::TAU)
                                .sin()
                                * 0.5;
                        t += 1;
                        std::iter::repeat_n(s, format.channels as usize)
                    })
                    .collect()
            })
            .collect()
    }

    fn energy(samples: &[f32]) -> f32 {
        samples.iter().map(|s| s * s).sum::<f32>() / samples.len().max(1) as f32
    }

    #[test]
    fn rejects_bad_formats() {
        assert!(Format::new(44100, 1).is_err());
        assert!(Format::new(16000, 3).is_err());
        assert_eq!(Format::new(16000, 1).unwrap().frame_size(), 320);
        assert_eq!(Format::new(48000, 2).unwrap().frame_len(), 1920);
    }

    #[test]
    fn voice_round_trip() {
        let format = Format::new(16000, 1).unwrap();
        let mut encoder = VoiceEncoder::new(format, 24000, true).unwrap();
        let mut decoder = VoiceDecoder::new(format).unwrap();
        let frames = tone(format, 50);
        let mut bytes = 0;
        let mut out_energy = 0.0;
        for (i, frame) in frames.iter().enumerate() {
            let packet = encoder.encode(frame).unwrap();
            bytes += packet.len();
            let pcm = decoder.decode(&packet).unwrap();
            assert_eq!(pcm.len(), format.frame_len());
            if i > 5 {
                out_energy += energy(&pcm);
            }
        }
        // ~24 kbit/s for one second of audio.
        assert!((2000..4500).contains(&bytes), "{bytes} bytes");
        // The tone comes through at about its original level (0.125).
        let level = out_energy / 44.0;
        assert!((0.06..0.2).contains(&level), "level {level}");
        assert!(encoder.encode(&[0.0; 10]).is_err());
    }

    #[test]
    fn lost_frames_are_concealed() {
        let format = Format::new(16000, 1).unwrap();
        let mut encoder = VoiceEncoder::new(format, 24000, true).unwrap();
        let mut decoder = VoiceDecoder::new(format).unwrap();
        let packets: Vec<Vec<u8>> = tone(format, 10)
            .iter()
            .map(|f| encoder.encode(f).unwrap())
            .collect();
        for packet in &packets[..5] {
            decoder.decode(packet).unwrap();
        }
        // Packet 5 is lost: rebuild it from packet 6's FEC, or by concealment.
        assert_eq!(
            decoder.conceal(Some(&packets[6])).unwrap().len(),
            format.frame_len()
        );
        assert_eq!(decoder.conceal(None).unwrap().len(), format.frame_len());
        assert_eq!(
            decoder.decode(&packets[6]).unwrap().len(),
            format.frame_len()
        );
    }
}
