use std::io::{self, Read};

use md5::{Digest, Md5};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hashes {
    pub crc32: String,
    pub md5: String,
    pub sha1: String,
}

/// Hashes a stream using a fixed-size buffer, without accumulating its contents.
pub fn hash_reader<R: Read>(mut reader: R) -> io::Result<Hashes> {
    let mut crc32 = crc32fast::Hasher::new();
    let mut md5 = Md5::new();
    let mut sha1 = sha1::Sha1::new();
    let mut buffer = [0_u8; 64 * 1024];

    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        let bytes = &buffer[..read];
        crc32.update(bytes);
        md5.update(bytes);
        sha1.update(bytes);
    }

    Ok(Hashes {
        crc32: format!("{:08x}", crc32.finalize()),
        md5: to_hex(&md5.finalize()),
        sha1: to_hex(&sha1.finalize()),
    })
}

fn to_hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes.iter().fold(
        String::with_capacity(bytes.len() * 2),
        |mut output, byte| {
            write!(output, "{byte:02x}").expect("writing to a String cannot fail");
            output
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_known_value() {
        let hashes = hash_reader("abc".as_bytes()).unwrap();
        assert_eq!(hashes.crc32, "352441c2");
        assert_eq!(hashes.md5, "900150983cd24fb0d6963f7d28e17f72");
        assert_eq!(hashes.sha1, "a9993e364706816aba3e25717850c26c9cd0d89d");
    }
}
