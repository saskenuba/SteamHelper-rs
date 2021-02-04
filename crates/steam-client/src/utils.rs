use steam_language_gen::generated::enums::EResult;
use steam_language_gen::FromPrimitive;

pub(crate) trait EResultCast: Copy {
    /// Should only be casted for methods such as `get_eresult`
    fn as_eresult(&self) -> EResult;
}

impl EResultCast for i32 {
    fn as_eresult(&self) -> EResult {
        EResult::from_i32(*self).unwrap()
    }
}

/// Read a valid utf8 string until the null terminator.
pub fn str_from_u8_nul_utf8(utf8_src: &[u8]) -> Result<&str, std::str::Utf8Error> {
    let nul_range_end = utf8_src.iter().position(|&c| c == b'\0').unwrap_or(utf8_src.len()); // default to length if no `\0` present
    ::std::str::from_utf8(&utf8_src[0..nul_range_end])
}
