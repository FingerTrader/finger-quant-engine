//! Private pipe framing; reject oversized length before allocation. No text logs on stdout.
use std::io::{self, Read, Write};
pub fn read_frame(mut input: impl Read) -> io::Result<Option<Vec<u8>>> {
    let mut header = [0; 4];
    match input.read(&mut header[..1])? {
        0 => return Ok(None),
        1 => {}
        _ => unreachable!(),
    }
    input.read_exact(&mut header[1..])?;
    let length = u32::from_be_bytes(header) as usize;
    if length == 0 || length > crate::MAX_REQUEST {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "request frame quota",
        ));
    }
    let mut bytes = vec![0; length];
    input.read_exact(&mut bytes)?;
    Ok(Some(bytes))
}
pub fn write_frame(mut output: impl Write, bytes: &[u8]) -> io::Result<()> {
    if bytes.is_empty() || bytes.len() > crate::MAX_RESPONSE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "response frame quota",
        ));
    }
    output.write_all(&(bytes.len() as u32).to_be_bytes())?;
    output.write_all(bytes)?;
    output.flush()
}
