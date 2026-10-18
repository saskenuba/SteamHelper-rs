//! Match share codes, such as `CSGO-vGqmL-vb2x2-nTQtU-e9dco-qWHzN`: what the game coordinator needs to find a match.

use std::convert::TryFrom;
use std::fmt;
use std::str::FromStr;

use thiserror::Error;

/// The characters of a share code, a digit each, in base 57: no `0`, `1`, `I`, `O`… that look alike.
const DICTIONARY: &[u8; 57] = b"ABCDEFGHJKLMNOPQRSTUVWXYZabcdefhijkmnopqrstuvwxyz23456789";
const BASE: u32 = DICTIONARY.len() as u32;
const PREFIX: &str = "CSGO";
/// The digits after the prefix, in groups of [`GROUP`].
const DIGITS: usize = 25;
const GROUP: usize = 5;
/// The bytes the digits hold: the match id, the outcome id and the token, each little-endian.
const BYTES: usize = 18;
/// The command a "download match" link runs, before the code.
const LINK_COMMAND: &str = "steam://rungame/730/76561202255233023/+csgo_download_match";

/// A match share code: the match, its outcome (reservation) and a token, which the game coordinator asks to give a
/// match's details (`MatchListRequestFullGameInfo`).
///
/// Reads `CSGO-xxxxx-xxxxx-xxxxx-xxxxx-xxxxx` (the dashes may be left out) and the
/// `steam://rungame/730/76561202255233023/+csgo_download_match%20CSGO-…` links; [`Display`](fmt::Display) writes the
/// code with its dashes.
///
/// ```
/// # fn main() -> Result<(), steam_client::gc::cs2::ShareCodeError> {
/// use steam_client::gc::cs2::ShareCode;
///
/// let code: ShareCode = "CSGO-vGqmL-vb2x2-nTQtU-e9dco-qWHzN".parse()?;
/// assert_eq!(code.match_id(), 3354591898827227340);
/// assert_eq!(code.outcome_id(), 3354598500191961136);
/// assert_eq!(code.token(), 7780);
/// assert_eq!(code.to_string(), "CSGO-vGqmL-vb2x2-nTQtU-e9dco-qWHzN");
/// # Ok(())
/// # }
/// ```
///
/// The code is a number of 144 bits written in base 57, least significant digit first, with the characters
/// `ABCDEFGHJKLMNOPQRSTUVWXYZabcdefhijkmnopqrstuvwxyz23456789`; its 18 bytes, most significant first, are the match id
/// (u64), the outcome id (u64) and the token (u16), each little-endian. As node-globaloffensive's
/// `globaloffensive-sharecode` and akiver's `csgo-sharecode` (which calls them the reservation id and the TV port) read
/// it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShareCode {
    match_id: u64,
    outcome_id: u64,
    token: u16,
}

impl ShareCode {
    /// The code of a match: its id, its outcome (reservation) id and its token (the TV port).
    pub fn new(match_id: u64, outcome_id: u64, token: u16) -> Self {
        Self {
            match_id,
            outcome_id,
            token,
        }
    }

    /// The match's id.
    pub fn match_id(&self) -> u64 {
        self.match_id
    }

    /// The match's outcome id: the reservation id of its last round.
    pub fn outcome_id(&self) -> u64 {
        self.outcome_id
    }

    /// The token: the port of the match's GOTV server.
    pub fn token(&self) -> u16 {
        self.token
    }

    /// The 144-bit number the code writes, most significant byte first.
    fn to_bytes(self) -> [u8; BYTES] {
        let mut bytes = [0; BYTES];
        bytes[..8].copy_from_slice(&self.match_id.to_le_bytes());
        bytes[8..16].copy_from_slice(&self.outcome_id.to_le_bytes());
        bytes[16..].copy_from_slice(&self.token.to_le_bytes());
        bytes
    }

    fn from_bytes(bytes: [u8; BYTES]) -> Self {
        let mut match_id = [0; 8];
        let mut outcome_id = [0; 8];
        match_id.copy_from_slice(&bytes[..8]);
        outcome_id.copy_from_slice(&bytes[8..16]);
        Self {
            match_id: u64::from_le_bytes(match_id),
            outcome_id: u64::from_le_bytes(outcome_id),
            token: u16::from_le_bytes([bytes[16], bytes[17]]),
        }
    }
}

impl FromStr for ShareCode {
    type Err = ShareCodeError;

    fn from_str(code: &str) -> Result<Self, Self::Err> {
        let code = code.trim();
        let code = match code.strip_prefix(LINK_COMMAND) {
            Some(argument) => argument
                .strip_prefix("%20")
                .or_else(|| argument.strip_prefix(' '))
                .ok_or(ShareCodeError::NotShareCode)?,
            None => code,
        };
        let mut characters = code
            .strip_prefix(PREFIX)
            .ok_or(ShareCodeError::NotShareCode)?
            .chars()
            .peekable();

        let mut digits = [0u8; DIGITS];
        for group in digits.chunks_mut(GROUP) {
            characters.next_if_eq(&'-');
            for digit in group {
                let character = characters.next().ok_or(ShareCodeError::Length)?;
                *digit = digit_of(character).ok_or(ShareCodeError::Character(character))?;
            }
        }
        if characters.next().is_some() {
            return Err(ShareCodeError::Length);
        }

        // the most significant digit is the last one
        let mut bytes = [0u8; BYTES];
        for &digit in digits.iter().rev() {
            let mut carry = u32::from(digit);
            for byte in bytes.iter_mut().rev() {
                let value = u32::from(*byte) * BASE + carry;
                *byte = value as u8;
                carry = value >> 8;
            }
            if carry != 0 {
                return Err(ShareCodeError::TooBig);
            }
        }
        Ok(Self::from_bytes(bytes))
    }
}

impl fmt::Display for ShareCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut bytes = self.to_bytes();
        f.write_str(PREFIX)?;
        for position in 0..DIGITS {
            if position % GROUP == 0 {
                f.write_str("-")?;
            }
            // divides the number by the base, keeping the remainder: the next digit
            let mut remainder = 0;
            for byte in bytes.iter_mut() {
                let value = remainder << 8 | u32::from(*byte);
                *byte = (value / BASE) as u8;
                remainder = value % BASE;
            }
            write!(f, "{}", char::from(DICTIONARY[remainder as usize]))?;
        }
        Ok(())
    }
}

fn digit_of(character: char) -> Option<u8> {
    let character = u8::try_from(character).ok()?;
    DICTIONARY
        .iter()
        .position(|&digit| digit == character)
        .map(|digit| digit as u8)
}

/// Why a string isn't a share code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum ShareCodeError {
    /// The string doesn't start with `CSGO`, nor is a "download match" link.
    #[error("Not a CS2 match share code.")]
    NotShareCode,
    /// The code doesn't have 25 characters after `CSGO`, in groups of 5 between dashes.
    #[error("The share code doesn't have 25 characters in groups of 5.")]
    Length,
    /// The code has a character that isn't one of the base 57 digits.
    #[error("The share code has the character {0:?}, which none has.")]
    Character(char),
    /// The code writes a number past the 144 bits it holds.
    #[error("The share code is too big.")]
    TooBig,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// From akiver's csgo-sharecode tests (`src/index.test.ts`), and node-globaloffensive's `globaloffensive-sharecode`
    /// README (the first).
    const SAMPLES: &[(&str, u64, u64, u16)] = &[
        (
            "CSGO-vGqmL-vb2x2-nTQtU-e9dco-qWHzN",
            3354591898827227340,
            3354598500191961136,
            7780,
        ),
        (
            "CSGO-L9spZ-ihuov-cyhtE-kxbqa-FkBAA",
            3400360672356205056,
            3400367402569957763,
            9725,
        ),
        (
            "CSGO-GADqf-jjyJ8-cSP2r-smZRo-TO2xK",
            3230642215713767580,
            3230647599455273103,
            55788,
        ),
        (
            "CSGO-bPQEz-PrYTq-u5w8E-ZbUy7-ZeQ3A",
            3325408798641750542,
            3325410334092558852,
            240,
        ),
        (
            "CSGO-wBrm6-7fkM6-AzBC5-u6GmR-iHLHA",
            3302232779302895618,
            3302241568953467250,
            3085,
        ),
        (
            "CSGO-TKDTJ-YrAXs-sDNfL-HOuKO-i84VH",
            3402250361329680757,
            3402250801563828781,
            61630,
        ),
        (
            "CSGO-p4X9o-3Mfut-tpe5y-J8K6f-mj5ZJ",
            3402249502336221574,
            3402252092201501292,
            14119,
        ),
    ];

    #[test]
    fn decodes_and_encodes_the_samples() {
        for &(text, match_id, outcome_id, token) in SAMPLES {
            let code: ShareCode = text.parse().unwrap();
            assert_eq!(code, ShareCode::new(match_id, outcome_id, token), "{}", text);
            assert_eq!(code.to_string(), text);
        }
    }

    #[test]
    fn reads_codes_without_dashes_and_download_links() {
        let code = ShareCode::new(3354591898827227340, 3354598500191961136, 7780);
        for text in [
            "CSGOvGqmLvb2x2nTQtUe9dcoqWHzN",
            "CSGO-vGqmLvb2x2-nTQtUe9dco-qWHzN",
            " CSGO-vGqmL-vb2x2-nTQtU-e9dco-qWHzN\n",
            "steam://rungame/730/76561202255233023/+csgo_download_match%20CSGO-vGqmL-vb2x2-nTQtU-e9dco-qWHzN",
            "steam://rungame/730/76561202255233023/+csgo_download_match CSGO-vGqmL-vb2x2-nTQtU-e9dco-qWHzN",
        ] {
            assert_eq!(text.parse(), Ok(code), "{}", text);
        }
    }

    #[test]
    fn round_trips() {
        for code in [
            ShareCode::new(0, 0, 0),
            ShareCode::new(u64::MAX, u64::MAX, u16::MAX),
            ShareCode::new(1, 2, 3),
            ShareCode::new(0x0123_4567_89AB_CDEF, 0xFEDC_BA98_7654_3210, 0x8001),
        ] {
            let text = code.to_string();
            assert_eq!(text.len(), 34);
            assert_eq!(text.parse(), Ok(code), "{}", text);
        }
        assert_eq!(
            ShareCode::new(0, 0, 0).to_string(),
            "CSGO-AAAAA-AAAAA-AAAAA-AAAAA-AAAAA"
        );
        // the biggest number the bytes hold
        assert_eq!(
            ShareCode::new(u64::MAX, u64::MAX, u16::MAX).to_string(),
            "CSGO-Acc38-83iaN-HmMno-7LBiJ-GwtGR"
        );
    }

    #[test]
    fn rejects_what_isnt_a_share_code() {
        use ShareCodeError::*;
        for (text, error) in [
            ("", NotShareCode),
            ("vGqmL-vb2x2-nTQtU-e9dco-qWHzN", NotShareCode),
            ("csgo-vGqmL-vb2x2-nTQtU-e9dco-qWHzN", NotShareCode),
            (
                "steam://rungame/730/76561202255233023/+csgo_download_matchCSGO-vGqmL",
                NotShareCode,
            ),
            // akiver's invalid samples
            ("CSGO-12345-12345-12345-12345-1234", Character('1')),
            ("whateverCSGO-12345-12345-12345-12345-12345", NotShareCode),
            ("CSGO-L9spZ-ihuov-cyhtE-kxbqa-FkBAAwhatever", Length),
            ("CSGO-11111-22222-33333-44444-55555", Character('1')),
            ("CSGO-vGqmL-vb2x2-nTQtU-e9dco-qWHz", Length),
            ("CSGO-vGqmL-vb2x2-nTQtU-e9dco-qWHzNA", Length),
            ("CSGO-vGqmL--vb2x2-nTQtU-e9dco-qWHzN", Character('-')),
            ("CSGO-vGqmL-vb2x2-nTQtU-e9dco-qWHzN-", Length),
            ("CSGO-vGqmL-vb2x2-nTQtU-e9dco-qWHz0", Character('0')),
            ("CSGO-vGqmL-vb2x2-nTQtU-e9dco-qWHzé", Character('é')),
            // one past the biggest
            ("CSGO-Bcc38-83iaN-HmMno-7LBiJ-GwtGR", TooBig),
            ("CSGO-99999-99999-99999-99999-99999", TooBig),
        ] {
            assert_eq!(text.parse::<ShareCode>(), Err(error), "{}", text);
        }
    }
}
