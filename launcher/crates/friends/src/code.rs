//! Invite codes: eight Crockford base32 symbols shown as `K7QM-3XPD`. The
//! first four are the nameplate the relay finds the host by; the last four
//! are the secret the key exchange uses, which only the players see.
use std::fmt;

/// Crockford base32: digits and letters without I, L, O and U.
pub const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
/// Symbols in each half of a code.
pub const HALF: usize = 4;

/// The value of a symbol as a player may type it: any case, `O` for `0`,
/// `I` and `L` for `1`.
fn value(c: char) -> Option<u8> {
    let c = match c.to_ascii_uppercase() {
        'O' => '0',
        'I' | 'L' => '1',
        c => c,
    };
    ALPHABET
        .iter()
        .position(|&a| char::from(a) == c)
        .map(|i| i as u8)
}

#[derive(Clone, PartialEq, Eq)]
pub struct Code {
    pub nameplate: String,
    pub secret: String,
}
impl Code {
    /// A code as a player typed it: spaces, dashes and case don't matter, `O`
    /// reads as `0`, `I` and `L` as `1`.
    pub fn parse(text: &str) -> Option<Self> {
        let mut out = String::with_capacity(2 * HALF);
        for c in text.chars() {
            if c.is_whitespace() || c == '-' {
                continue;
            }
            out.push(char::from(ALPHABET[value(c)? as usize]));
        }
        if out.len() != 2 * HALF {
            return None;
        }
        let (nameplate, secret) = out.split_at(HALF);
        Some(Self {
            nameplate: nameplate.to_owned(),
            secret: secret.to_owned(),
        })
    }
}
impl fmt::Display for Code {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}-{}", self.nameplate, self.secret)
    }
}
/// The secret stays out of debug output and logs.
impl fmt::Debug for Code {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Code({}-****)", self.nameplate)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alphabet_has_no_ambiguous_letters() {
        assert_eq!(ALPHABET.len(), 32);
        for c in *b"ILOU" {
            assert!(!ALPHABET.contains(&c));
        }
        let mut sorted = ALPHABET.to_vec();
        sorted.dedup();
        assert_eq!(sorted.len(), 32);
    }

    #[test]
    fn parses_what_players_type() {
        let code = Code::parse("k7qm-3xpd").unwrap();
        assert_eq!(code.nameplate, "K7QM");
        assert_eq!(code.secret, "3XPD");
        assert_eq!(code.to_string(), "K7QM-3XPD");
        assert_eq!(Code::parse(" K7QM 3XPD ").unwrap(), code);
        assert_eq!(Code::parse("K7QM3XPD").unwrap(), code);
        // O, I and L are the symbols they look like.
        let code = Code::parse("OIL0-1lio").unwrap();
        assert_eq!(code.to_string(), "0110-1110");
        assert!(Code::parse("K7QM-3XP").is_none());
        assert!(Code::parse("K7QM-3XPDA").is_none());
        assert!(Code::parse("K7QM-3XPU").is_none());
        assert!(Code::parse("").is_none());
    }
}
