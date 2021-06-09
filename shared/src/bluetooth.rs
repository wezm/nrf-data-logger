use core::str;

pub fn fmt_addr<'buf>(addr: &[u8; 6], addr_str: &'buf mut [u8; 12 + 5]) -> &'buf str {
    *addr_str = [b':'; 12 + 5];
    for (i, byte) in addr.iter().copied().rev().enumerate() {
        addr_str[i * 3] = char::from_digit((u32::from(byte) & 0xF0) >> 4, 16)
            .unwrap()
            .to_ascii_uppercase() as u8;
        addr_str[i * 3 + 1] = char::from_digit(u32::from(byte) & 0xF, 16)
            .unwrap()
            .to_ascii_uppercase() as u8;
    }
    str::from_utf8(addr_str).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fmt_addr() {
        let addr = [0x24, 0xBE, 0x59, 0x38, 0xC1, 0xA4];
        let mut buf = [0; 12 + 5];
        let addr_str = fmt_addr(&addr, &mut buf);
        assert_eq!(addr_str, "A4:C1:38:59:BE:24");
    }
}
