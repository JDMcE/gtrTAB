//! Sound written to a WAV file: 16-bit stereo, which every program that plays or edits
//! audio reads.

/// The bytes of a WAV file holding two channels of samples from -1.0 to 1.0. A channel
/// shorter than the other is silent to the end.
///
/// Rounding to sixteen bits is done with a little noise added first, as is usual: without
/// it, a note that dies away ends in the grinding of its last few steps.
pub fn wav(left: &[f32], right: &[f32], sample_rate: u32) -> Vec<u8> {
    const CHANNELS: u16 = 2;
    const BYTES: u16 = 2;
    let frames = left.len().max(right.len());
    // The format has four bytes for its length; what does not fit is left off.
    let most = (u32::MAX as usize - 44) / usize::from(CHANNELS * BYTES);
    let frames = frames.min(most);
    let data = (frames * usize::from(CHANNELS * BYTES)) as u32;

    let mut out = Vec::with_capacity(44 + data as usize);
    out.extend(b"RIFF");
    out.extend((36 + data).to_le_bytes());
    out.extend(b"WAVEfmt ");
    out.extend(16u32.to_le_bytes());
    out.extend(1u16.to_le_bytes()); // plain samples, not compressed
    out.extend(CHANNELS.to_le_bytes());
    out.extend(sample_rate.to_le_bytes());
    out.extend((sample_rate * u32::from(CHANNELS * BYTES)).to_le_bytes());
    out.extend((CHANNELS * BYTES).to_le_bytes());
    out.extend((BYTES * 8).to_le_bytes());
    out.extend(b"data");
    out.extend(data.to_le_bytes());

    // Two random numbers a sample, from -0.5 to 0.5 of a step each, make noise that is
    // spread like a triangle; a simple generator is random enough for that.
    let mut state = 0x2545_F491u32;
    let mut noise = move || {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (state >> 8) as f32 / (1u32 << 24) as f32 - 0.5
    };
    for frame in 0..frames {
        for side in [left, right] {
            let sample = side.get(frame).copied().unwrap_or(0.0);
            let steps = sample.clamp(-1.0, 1.0) * 32_767.0 + noise() + noise();
            out.extend((steps.round().clamp(-32_768.0, 32_767.0) as i16).to_le_bytes());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn samples(file: &[u8]) -> Vec<(i16, i16)> {
        (file[44..].as_chunks::<4>().0.iter())
            .map(|f| {
                (
                    i16::from_le_bytes([f[0], f[1]]),
                    i16::from_le_bytes([f[2], f[3]]),
                )
            })
            .collect()
    }

    #[test]
    fn a_wav_file_has_its_header_and_its_samples() {
        let left: Vec<f32> = (0..1000).map(|i| (i as f32 * 0.05).sin() * 0.5).collect();
        let right = vec![-1.0, 1.0, 2.0, 0.25];
        let file = wav(&left, &right, 48_000);
        assert_eq!(&file[..4], b"RIFF");
        assert_eq!(&file[8..16], b"WAVEfmt ");
        let u32_at = |at: usize| u32::from_le_bytes(file[at..at + 4].try_into().unwrap());
        let u16_at = |at: usize| u16::from_le_bytes(file[at..at + 2].try_into().unwrap());
        // One format, two channels, the rate, bytes a second, bytes a frame, bits a sample.
        assert_eq!((u16_at(20), u16_at(22), u32_at(24)), (1, 2, 48_000));
        assert_eq!((u32_at(28), u16_at(32), u16_at(34)), (192_000, 4, 16));
        assert_eq!(&file[36..40], b"data");
        assert_eq!(u32_at(40) as usize, 1000 * 4);
        assert_eq!(u32_at(4) as usize, file.len() - 8);

        let read = samples(&file);
        assert_eq!(read.len(), 1000);
        // Each sample is within the noise of where it should be, and the noise is there.
        for (i, &(l, _)) in read.iter().enumerate() {
            assert!(
                (f32::from(l) - left[i] * 32_767.0).abs() <= 1.5,
                "sample {i}"
            );
        }
        assert!(
            read.iter()
                .enumerate()
                .any(|(i, &(l, _))| f32::from(l) != (left[i] * 32_767.0).round())
        );
        // What is out of range is held at the edge, and the shorter side ends in silence.
        assert!(read[0].1 <= -32_766 && read[1].1 >= 32_766 && read[2].1 >= 32_766);
        assert!(read[10..].iter().all(|&(_, r)| r.abs() <= 1));
        assert_eq!(wav(&[], &[], 44_100).len(), 44);
    }
}
