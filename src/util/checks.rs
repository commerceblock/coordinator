//! checks
//!
//! validity checks for string inputs. Use before adding to Config

/// Return true if char is in base58check character set, false otherwise
fn is_base58_char(char: &char) -> bool {
    match *char as u8 {
        b'0'..=b'9' | b'A'..=b'H' | b'J'..=b'N' | b'P'..=b'Z' | b'a'..=b'k' | b'm'..=b'z' => true,
        _ => false,
    }
}

/// Check for correct priv key input string format
pub fn check_privkey_string(str: &String) -> bool {
    if str.len() == 52 && str.chars().all(|x| is_base58_char(&x)) {
        return true;
    }
    return false;
}

/// Check for correct hash input string format
pub fn check_hash_string(str: &String) -> bool {
    if str.len() == 64 && str.chars().all(|x| x.is_ascii_hexdigit()) {
        return true;
    }
    return false;
}
