//! Parser combinator functions similar to `nom` but less generic.

use crate::{
    error::{EepromError, Error, PduError},
    fmt,
};

pub fn map<T, U>((i, value): (&[u8], T), f: impl FnOnce(T) -> U) -> (&[u8], U) {
    (i, f(value))
}

pub fn map_res<T, U, F, E>((i, value): (&[u8], T), f: F) -> Result<(&[u8], U), Error>
where
    F: FnOnce(T) -> Result<U, E>,
    E: Into<Error>,
{
    f(value).map(|value| (i, value)).map_err(|e| e.into())
}

pub fn map_opt<T, U>(
    (i, value): (&[u8], T),
    f: impl FnOnce(T) -> Option<U>,
) -> Result<(&[u8], U), Error> {
    f(value)
        .map(|res| (i, res))
        .ok_or(EepromError::Decode.into())
}

pub fn new_all_consumed(i: &[u8]) -> Result<(), Error> {
    if i.is_empty() {
        return Ok(());
    }

    Err(PduError::Decode.into())
}

pub fn new_le_u16(i: &[u8]) -> Result<(&[u8], u16), Error> {
    if i.len() < 2 {
        Err(EepromError::Decode)?;
    }

    let (raw, rest) = i.split_at(2);

    let value = u16::from_le_bytes(fmt::unwrap!(raw.try_into()));

    Ok((rest, value))
}

pub fn new_le_u32(i: &[u8]) -> Result<(&[u8], u32), Error> {
    if i.len() < 4 {
        Err(EepromError::Decode)?;
    }

    let (raw, rest) = i.split_at(4);

    let value = u32::from_le_bytes(fmt::unwrap!(raw.try_into()));

    Ok((rest, value))
}

pub fn new_le_i16(i: &[u8]) -> Result<(&[u8], i16), Error> {
    if i.len() < 2 {
        Err(EepromError::Decode)?;
    }

    let (raw, rest) = i.split_at(2);

    let value = i16::from_le_bytes(fmt::unwrap!(raw.try_into()));

    Ok((rest, value))
}

pub fn new_le_u8(i: &[u8]) -> Result<(&[u8], u8), Error> {
    if i.is_empty() {
        Err(EepromError::Decode)?;
    }

    let Some((first, rest)) = i.split_first() else {
        return Err(EepromError::Decode.into());
    };

    Ok((rest, *first))
}
