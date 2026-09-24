//! Encodages : détection (BOM + heuristiques) et conversion.
//!
//! TextMate gérait les encodages via un catalogue de `charsets` ; il détectait
//! la préférence via le BOM et une heuristique de vraisemblance. On reproduit
//! ici l'essentiel : BOM UTF-8 / UTF-16, validité UTF-8, heuristique UTF-16
//! sans BOM, et repli Windows-1252 (mappage complet, pas seulement Latin-1).

/// Un encodage de fichier supporté.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    /// UTF-8 sans BOM (cas le plus courant).
    Utf8,
    /// UTF-8 avec BOM `EF BB BF`.
    Utf8Bom,
    /// UTF-16 little-endian (la détection suppose un BOM).
    Utf16Le,
    /// UTF-16 big-endian (la détection suppose un BOM).
    Utf16Be,
    /// Windows-1252 : superset de Latin-1 utilisé pour le repli quand
    /// les octets ne forment pas de l'UTF-8 valide.
    Windows1252,
}

impl Encoding {
    /// Nom lisible pour la barre d'état.
    pub fn name(self) -> &'static str {
        match self {
            Encoding::Utf8 => "UTF-8",
            Encoding::Utf8Bom => "UTF-8 (BOM)",
            Encoding::Utf16Le => "UTF-16 LE",
            Encoding::Utf16Be => "UTF-16 BE",
            Encoding::Windows1252 => "Windows-1252",
        }
    }
}

/// Détecte l'encodage le plus probable d'un fichier.
///
/// Ordre de priorité :
/// 1. BOM présent (UTF-8, UTF-16 LE/BE) — décision certaine ;
/// 2. octets valides UTF-8 — `Utf8` ;
/// 3. motif d'octets nuls alternés sur les 64 premiers octets — UTF-16
///    sans BOM (heuristique, comme l'ancien Notepad) ;
/// 4. sinon `Windows1252`.
pub fn detect(bytes: &[u8]) -> Encoding {
    // 1. BOM
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return Encoding::Utf8Bom;
    }
    if bytes.starts_with(&[0xFF, 0xFE]) {
        return Encoding::Utf16Le;
    }
    if bytes.starts_with(&[0xFE, 0xFF]) {
        return Encoding::Utf16Be;
    }
    // 2. Heuristique UTF-16 sans BOM : les unités UTF-16 en ASCII ont un
    // octet à zéro sur deux (0x48 0x00 0x65 0x00…). La sonde passe avant
    // le test de validité UTF-8, car les octets nuls sont valides en UTF-8
    // et masqueraient ce motif. On sonde le début du fichier.
    let probe_len = bytes.len().min(64);
    let (mut even_zero, mut odd_zero) = (0usize, 0usize);
    for (i, b) in bytes[..probe_len].iter().enumerate() {
        if *b == 0 {
            if i % 2 == 0 { even_zero += 1 } else { odd_zero += 1 }
        }
    }
    let needed = probe_len / 4;
    if odd_zero >= needed && odd_zero > even_zero {
        return Encoding::Utf16Le;
    }
    if even_zero >= needed && even_zero > odd_zero {
        return Encoding::Utf16Be;
    }
    // 3. UTF-8 valide
    if std::str::from_utf8(bytes).is_ok() {
        return Encoding::Utf8;
    }
    // 4. Repli
    Encoding::Windows1252
}

/// Décode des octets dans l'encodage donné.
///
/// Les BOM sont retirés ; les séquences invalides sont remplacées par
/// `U+FFFD` plutôt que de faire échouer l'ouverture.
pub fn decode(bytes: &[u8], enc: Encoding) -> String {
    match enc {
        Encoding::Utf8 => String::from_utf8_lossy(bytes).into_owned(),
        Encoding::Utf8Bom => String::from_utf8_lossy(&bytes[3..]).into_owned(),
        Encoding::Utf16Le => decode_utf16(bytes, false),
        Encoding::Utf16Be => decode_utf16(bytes, true),
        Encoding::Windows1252 => bytes.iter().map(|&b| cp1252_decode(b)).collect(),
    }
}

/// Encode un texte dans l'encodage donné (BOM réinsérés).
pub fn encode(text: &str, enc: Encoding) -> Vec<u8> {
    match enc {
        Encoding::Utf8 => text.as_bytes().to_vec(),
        Encoding::Utf8Bom => {
            let mut v = vec![0xEF, 0xBB, 0xBF];
            v.extend_from_slice(text.as_bytes());
            v
        }
        Encoding::Utf16Le => {
            let mut v = vec![0xFF, 0xFE];
            for u in text.encode_utf16() {
                v.extend_from_slice(&u.to_le_bytes());
            }
            v
        }
        Encoding::Utf16Be => {
            let mut v = vec![0xFE, 0xFF];
            for u in text.encode_utf16() {
                v.extend_from_slice(&u.to_be_bytes());
            }
            v
        }
        Encoding::Windows1252 => text.chars().flat_map(cp1252_encode).collect(),
    }
}

// ===================
// = UTF-16 ==========
// ===================

fn decode_utf16(bytes: &[u8], big_endian: bool) -> String {
    let mut units = Vec::new();
    let mut rest = bytes;
    // Retire le BOM éventuel sans faire d'hypothèse sur son encodage
    if rest.len() >= 2 && ((rest[0] == 0xFF && rest[1] == 0xFE) || (rest[0] == 0xFE && rest[1] == 0xFF)) {
        rest = &rest[2..];
    }
    for chunk in rest.chunks_exact(2) {
        let u = if big_endian {
            u16::from_be_bytes([chunk[0], chunk[1]])
        } else {
            u16::from_le_bytes([chunk[0], chunk[1]])
        };
        units.push(u);
    }
    // un octet restant isolé est ignoré (fichier tronqué)
    String::from_utf16_lossy(&units)
}

// ===================
// = Windows-1252 ====
// ===================

fn cp1252_decode(byte: u8) -> char {
    match byte {
        0x80 => '€', 0x82 => '‚', 0x83 => 'ƒ', 0x84 => '„',
        0x85 => '…', 0x86 => '†', 0x87 => '‡', 0x88 => 'ˆ',
        0x89 => '‰', 0x8A => 'Š', 0x8B => '‹', 0x8C => 'Œ',
        0x8E => 'Ž', 0x91 => '‘', 0x92 => '’', 0x93 => '“',
        0x94 => '”', 0x95 => '•', 0x96 => '–', 0x97 => '—',
        0x98 => '˜', 0x99 => '™', 0x9A => 'š', 0x9B => '›',
        0x9C => 'œ', 0x9E => 'ž', 0x9F => 'Ÿ',
        _ => char::from(byte),
    }
}

fn cp1252_encode(c: char) -> Vec<u8> {
    let byte = match c {
        '€' => Some(0x80), '‚' => Some(0x82), 'ƒ' => Some(0x83), '„' => Some(0x84),
        '…' => Some(0x85), '†' => Some(0x86), '‡' => Some(0x87), 'ˆ' => Some(0x88),
        '‰' => Some(0x89), 'Š' => Some(0x8A), '‹' => Some(0x8B), 'Œ' => Some(0x8C),
        'Ž' => Some(0x8E), '‘' => Some(0x91), '’' => Some(0x92), '“' => Some(0x93),
        '”' => Some(0x94), '•' => Some(0x95), '–' => Some(0x96), '—' => Some(0x97),
        '˜' => Some(0x98), '™' => Some(0x99), 'š' => Some(0x9A), '›' => Some(0x9B),
        'œ' => Some(0x9C), 'ž' => Some(0x9E), 'Ÿ' => Some(0x9F),
        _ if (c as u32) <= 0xFF => Some(c as u8),
        _ => None, // hors Windows-1252 : remplacé par « ? » (comportement TextMate)
    };
    match byte {
        Some(b) => vec![b],
        None => vec![b'?'],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_boms() {
        assert_eq!(detect(b"plain ascii"), Encoding::Utf8);
        assert_eq!(detect(&[0xEF, 0xBB, 0xBF, b'a', b'b']), Encoding::Utf8Bom);
        // "é" en UTF-8 : 0xC3 0xA9 — valide
        assert_eq!(detect(&[0xC3, 0xA9, b' ', b'x']), Encoding::Utf8);
        assert_eq!(detect(&[0xFF, 0xFE, 0x61, 0x00]), Encoding::Utf16Le);
        assert_eq!(detect(&[0xFE, 0xFF, 0x00, 0x61]), Encoding::Utf16Be);
    }

    #[test]
    fn detect_utf16_without_bom_heuristic() {
        // "Hello" en UTF-16LE sans BOM : octets nuls impairs
        assert_eq!(detect(b"H\x00e\x00l\x00l\x00o\x00"), Encoding::Utf16Le);
        // en UTF-16BE : octets nuls pairs
        assert_eq!(detect(b"\x00H\x00e\x00l\x00l\x00o"), Encoding::Utf16Be);
        // UTF-8 multi-octets invalide → repli Windows-1252
        assert_eq!(detect(&[0xE9, b' ', 0xE8, b' ']), Encoding::Windows1252);
    }

    #[test]
    fn utf8_bom_roundtrip() {
        let enc = Encoding::Utf8Bom;
        let bytes = encode("café", enc);
        assert_eq!(bytes, vec![0xEF, 0xBB, 0xBF, b'c', b'a', b'f', 0xC3, 0xA9]);
        assert_eq!(decode(&bytes, enc), "café");
    }

    #[test]
    fn utf16_roundtrips() {
        let text = "héllo 🎉 wörld";
        for enc in [Encoding::Utf16Le, Encoding::Utf16Be] {
            let bytes = encode(text, enc);
            assert!(bytes.starts_with(&[0xFF, 0xFE]) || bytes.starts_with(&[0xFE, 0xFF]));
            assert_eq!(decode(&bytes, enc), text);
        }
    }

    #[test]
    fn utf16_tolerates_truncated_trailing_byte() {
        // unité UTF-16 coupée en deux : l'octet orphelin est ignoré
        let bytes = encode("ab", Encoding::Utf16Le);
        assert_eq!(decode(&bytes[..bytes.len() - 1], Encoding::Utf16Le), "a");
        assert_eq!(decode(&bytes[..bytes.len() - 3], Encoding::Utf16Le), "");
    }

    #[test]
    fn cp1252_handles_smart_quotes_and_euro() {
        // “café” 0x93 63 61 66 E9 0x94
        let bytes = [0x93, b'c', b'a', b'f', 0xE9, 0x94];
        assert_eq!(detect(&bytes), Encoding::Windows1252);
        let text = decode(&bytes, Encoding::Windows1252);
        assert_eq!(text, "“café”");
        // roundtrip : l'encodage redonne exactement les mêmes octets
        assert_eq!(encode(&text, Encoding::Windows1252), bytes);
    }

    #[test]
    fn cp1252_unencodable_char_falls_back() {
        // 🎉 n'existe pas en Windows-1252 → « ? »
        assert_eq!(encode("a🎉b", Encoding::Windows1252), b"a?b");
    }
}