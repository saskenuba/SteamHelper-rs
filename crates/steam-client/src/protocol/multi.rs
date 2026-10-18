//! `EMsg::Multi`: several messages sent as one, often gzip compressed.
//!
//! The packets are slices of the Multi's body, or of its unzipped body: unpacking copies nothing, and gunzipping
//! allocates only the buffer it unzips into.
//!
//! Reference: https://github.com/SteamRE/SteamKit/blob/master/SteamKit2/SteamKit2/Steam/CMClient.cs (`HandleMulti`)

use std::collections::VecDeque;

use bytes::Buf;
use bytes::Bytes;
use miniz_oxide::inflate::core::decompress;
use miniz_oxide::inflate::core::inflate_flags::TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF;
use miniz_oxide::inflate::core::DecompressorOxide;
use miniz_oxide::inflate::TINFLStatus;
use steam_protobuf::protobufs::steammessages_base::CMsgMulti;
use thiserror::Error;

/// The most deflate can inflate its input by.
const MAX_INFLATE_RATIO: usize = 1032;
/// CRC32 and size of the unzipped data, after the deflate stream.
const GZIP_TRAILER_SIZE: usize = 8;

/// Why a `Multi` could not be unpacked.
#[derive(Debug, Error, PartialEq, Eq)]
pub(crate) enum MultiError {
    #[error("a packet in the Multi message is truncated")]
    Truncated,
    #[error("the Multi message body is not valid gzip: {0}")]
    Gzip(&'static str),
    #[error("the Multi message body does not unzip to the {0} bytes it declares")]
    UnzippedSize(u32),
}

/// Queues the raw packets a `CMsgMulti` carries in front of `pending`, in order: they come before anything read after
/// the Multi, and before what's left of an outer Multi. Returns how many there were.
///
/// The body is gzip compressed when `size_unzipped` is set, and is a sequence of packets, each prefixed by its length
/// as a u32. Queues nothing if any of it is invalid.
pub(crate) fn unpack_multi(mut multi: CMsgMulti, pending: &mut VecDeque<Bytes>) -> Result<usize, MultiError> {
    let body = multi.message_body.take().unwrap_or_default();
    let body = match multi.size_unzipped() {
        0 => body,
        size => gunzip(&body, size)?,
    };

    let queued = pending.len();
    for packet in Packets(body) {
        match packet {
            Ok(packet) => pending.push_back(packet),
            Err(error) => {
                pending.truncate(queued);
                return Err(error);
            }
        }
    }
    let unpacked = pending.len() - queued;
    pending.rotate_right(unpacked);
    Ok(unpacked)
}

/// The length prefixed packets of a Multi's body, as slices of it.
struct Packets(Bytes);

impl Iterator for Packets {
    type Item = Result<Bytes, MultiError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.0.is_empty() {
            return None;
        }
        if self.0.len() < 4 {
            self.0 = Bytes::new();
            return Some(Err(MultiError::Truncated));
        }
        let length = self.0.get_u32_le() as usize;
        if self.0.len() < length {
            self.0 = Bytes::new();
            return Some(Err(MultiError::Truncated));
        }
        Some(Ok(self.0.split_to(length)))
    }
}

/// Gunzips `body` into a buffer of `size` bytes, its declared unzipped size, which it must match.
fn gunzip(body: &[u8], size: u32) -> Result<Bytes, MultiError> {
    let deflated = skip_gzip_header(body)?;
    // checked before allocating, so a hostile size costs nothing
    if !can_inflate_to(deflated.len(), size) {
        return Err(MultiError::UnzippedSize(size));
    }

    let mut unzipped = vec![0; size as usize];
    let (status, read, written) = decompress(
        &mut DecompressorOxide::new(),
        deflated,
        &mut unzipped,
        0,
        TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF,
    );
    match status {
        TINFLStatus::Done if written == unzipped.len() => {}
        TINFLStatus::Done | TINFLStatus::HasMoreOutput => return Err(MultiError::UnzippedSize(size)),
        _ => return Err(MultiError::Gzip("invalid deflate data")),
    }

    let trailer = &deflated[read..];
    if trailer.len() != GZIP_TRAILER_SIZE {
        return Err(MultiError::Gzip("invalid trailer"));
    }
    if trailer[..4] != crc32fast::hash(&unzipped).to_le_bytes() {
        return Err(MultiError::Gzip("CRC mismatch"));
    }
    if trailer[4..] != size.to_le_bytes() {
        return Err(MultiError::UnzippedSize(size));
    }
    Ok(unzipped.into())
}

/// Whether `deflated` bytes can inflate to `size` bytes: deflate's best ratio is [`MAX_INFLATE_RATIO`].
fn can_inflate_to(deflated: usize, size: u32) -> bool {
    size as usize <= deflated.saturating_mul(MAX_INFLATE_RATIO)
}

/// Skips the header of a gzip member (RFC 1952). Returns what follows: the deflate stream, then the trailer.
fn skip_gzip_header(mut data: &[u8]) -> Result<&[u8], MultiError> {
    const FHCRC: u8 = 1 << 1;
    const FEXTRA: u8 = 1 << 2;
    const FNAME: u8 = 1 << 3;
    const FCOMMENT: u8 = 1 << 4;
    const RESERVED: u8 = 0b1110_0000;

    let header = take(&mut data, 10)?;
    // magic bytes, then the compression method: deflate
    if header[..3] != [0x1F, 0x8B, 8] {
        return Err(MultiError::Gzip("not a gzip header"));
    }
    let flags = header[3];
    if flags & RESERVED != 0 {
        return Err(MultiError::Gzip("reserved header flags are set"));
    }

    if flags & FEXTRA != 0 {
        let length = take(&mut data, 2)?;
        take(&mut data, u16::from_le_bytes([length[0], length[1]]) as usize)?;
    }
    for flag in [FNAME, FCOMMENT] {
        if flags & flag != 0 {
            // zero terminated
            let end = data.iter().position(|&byte| byte == 0).ok_or(TRUNCATED_HEADER)?;
            data = &data[end + 1..];
        }
    }
    if flags & FHCRC != 0 {
        take(&mut data, 2)?;
    }
    Ok(data)
}

const TRUNCATED_HEADER: MultiError = MultiError::Gzip("truncated header");

fn take<'a>(data: &mut &'a [u8], count: usize) -> Result<&'a [u8], MultiError> {
    if data.len() < count {
        return Err(TRUNCATED_HEADER);
    }
    let (taken, rest) = data.split_at(count);
    *data = rest;
    Ok(taken)
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use flate2::write::DeflateEncoder;
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use flate2::GzBuilder;

    use super::*;

    #[test]
    fn sizes_past_deflates_best_ratio_are_refused() {
        assert!(can_inflate_to(10, 10 * MAX_INFLATE_RATIO as u32));
        assert!(!can_inflate_to(10, 10 * MAX_INFLATE_RATIO as u32 + 1));
        assert!(!can_inflate_to(0, 1));
        assert!(can_inflate_to(usize::MAX, u32::MAX));
    }

    fn packed(packets: &[&[u8]]) -> Vec<u8> {
        let mut body = vec![];
        for packet in packets {
            body.extend_from_slice(&(packet.len() as u32).to_le_bytes());
            body.extend_from_slice(packet);
        }
        body
    }

    fn gzip(data: &[u8]) -> Vec<u8> {
        let mut encoder = GzEncoder::new(vec![], Compression::default());
        encoder.write_all(data).unwrap();
        encoder.finish().unwrap()
    }

    fn multi(body: Vec<u8>, size_unzipped: Option<u32>) -> CMsgMulti {
        let mut multi = CMsgMulti::new();
        if let Some(size) = size_unzipped {
            multi.set_size_unzipped(size);
        }
        multi.set_message_body(body.into());
        multi
    }

    fn unpack(multi: CMsgMulti) -> Result<Vec<Bytes>, MultiError> {
        let mut pending = VecDeque::new();
        unpack_multi(multi, &mut pending)?;
        Ok(pending.into())
    }

    #[test]
    fn unpacks_a_gzipped_multi() {
        let body = packed(&[b"first packet", b"", b"third"]);
        let multi = multi(gzip(&body), Some(body.len() as u32));

        assert_eq!(unpack(multi).unwrap(), [&b"first packet"[..], b"", b"third"]);
    }

    #[test]
    fn unpacks_a_raw_multi() {
        assert_eq!(unpack(multi(packed(&[b"only one"]), None)).unwrap(), [&b"only one"[..]]);
        assert!(unpack(CMsgMulti::new()).unwrap().is_empty());
    }

    #[test]
    fn queues_the_packets_before_the_pending_ones() {
        let mut pending = VecDeque::from([Bytes::from_static(b"pending")]);
        let unpacked = unpack_multi(multi(packed(&[b"first", b"second"]), None), &mut pending).unwrap();

        assert_eq!(unpacked, 2);
        assert_eq!(pending, [&b"first"[..], b"second", b"pending"]);
    }

    #[test]
    fn truncated_multis_are_errors() {
        let body = packed(&[b"a packet"]);
        for cut in [1, 4, body.len() - 1] {
            let mut pending = VecDeque::from([Bytes::from_static(b"pending")]);
            let truncated = [&body[..], &body[..cut]].concat();
            assert_eq!(
                unpack_multi(multi(truncated, None), &mut pending),
                Err(MultiError::Truncated)
            );
            // nothing queued
            assert_eq!(pending, [&b"pending"[..]]);
        }
    }

    #[test]
    fn reads_optional_gzip_header_fields() {
        let body = packed(&[b"a packet"]);
        let mut encoder = GzBuilder::new()
            .extra(vec![1, 2, 3])
            .filename("multi")
            .comment("comment")
            .write(vec![], Compression::default());
        encoder.write_all(&body).unwrap();
        let gzipped = multi(encoder.finish().unwrap(), Some(body.len() as u32));
        assert_eq!(unpack(gzipped).unwrap(), [&b"a packet"[..]]);

        // flate2 doesn't write a header CRC: build one by hand
        let mut deflate = DeflateEncoder::new(vec![], Compression::default());
        deflate.write_all(&body).unwrap();
        let mut gzipped = vec![0x1F, 0x8B, 8, 1 << 1, 0, 0, 0, 0, 0, 0xFF, 0xAB, 0xCD];
        gzipped.extend(deflate.finish().unwrap());
        gzipped.extend_from_slice(&crc32fast::hash(&body).to_le_bytes());
        gzipped.extend_from_slice(&(body.len() as u32).to_le_bytes());
        assert_eq!(
            unpack(multi(gzipped, Some(body.len() as u32))).unwrap(),
            [&b"a packet"[..]]
        );
    }

    #[test]
    fn rejects_truncated_optional_header_fields() {
        for header in [
            // the extra field's length, then fewer bytes than it says
            &[0x1F, 0x8B, 8, 1 << 2, 0, 0, 0, 0, 0, 0xFF, 1][..],
            &[0x1F, 0x8B, 8, 1 << 2, 0, 0, 0, 0, 0, 0xFF, 4, 0, 1][..],
            // half a header CRC
            &[0x1F, 0x8B, 8, 1 << 1, 0, 0, 0, 0, 0, 0xFF, 0xAB][..],
        ] {
            assert_eq!(skip_gzip_header(header), Err(TRUNCATED_HEADER), "{:?}", header);
        }
    }

    #[test]
    fn checks_the_unzipped_size() {
        let body = packed(&[b"a packet"]);
        let size = body.len() as u32;
        for declared in [size - 1, size + 1, u32::MAX] {
            assert_eq!(
                unpack(multi(gzip(&body), Some(declared))),
                Err(MultiError::UnzippedSize(declared))
            );
        }

        // the size in the trailer must match too
        let mut gzipped = gzip(&body);
        let len = gzipped.len();
        gzipped[len - 4..].copy_from_slice(&(size + 1).to_le_bytes());
        assert_eq!(unpack(multi(gzipped, Some(size))), Err(MultiError::UnzippedSize(size)));
    }

    #[test]
    fn invalid_gzip_is_an_error() {
        let body = packed(&[b"a packet"]);
        let gzipped = gzip(&body);
        let size = Some(body.len() as u32);
        let invalid = |gzipped: Vec<u8>| unpack(multi(gzipped, size)).unwrap_err();

        assert_eq!(invalid(body.clone()), MultiError::Gzip("not a gzip header"));
        assert_eq!(invalid(gzipped[..9].to_vec()), MultiError::Gzip("truncated header"));

        let mut reserved = gzipped.clone();
        reserved[3] = 1 << 5;
        assert_eq!(invalid(reserved), MultiError::Gzip("reserved header flags are set"));

        for flag in [1 << 2, 1 << 3] {
            let mut header = gzipped[..10].to_vec();
            header[3] = flag;
            assert_eq!(invalid(header), MultiError::Gzip("truncated header"));
        }

        let mut deflate = gzipped.clone();
        deflate[10] = 0xFF;
        assert_eq!(invalid(deflate), MultiError::Gzip("invalid deflate data"));

        assert_eq!(
            invalid(gzipped[..gzipped.len() - 1].to_vec()),
            MultiError::Gzip("invalid trailer")
        );
        assert_eq!(
            invalid([&gzipped[..], &[0]].concat()),
            MultiError::Gzip("invalid trailer")
        );

        let mut crc = gzipped;
        let len = crc.len();
        crc[len - 8] ^= 1;
        assert_eq!(invalid(crc), MultiError::Gzip("CRC mismatch"));
    }
}
