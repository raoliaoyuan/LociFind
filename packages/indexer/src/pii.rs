//! 轻量 PII 类型识别：只输出类型关键词，不返回、不落库识别到的号码本身。

/// 中国大陆身份证号命中后注入的 FTS 类型关键词。
pub(crate) const IDENTITY_CARD_KEYWORDS: &str = "身份证 身份证号 证件号 identity_card";
/// 中国大陆手机号命中后注入的 FTS 类型关键词。
pub(crate) const PHONE_KEYWORDS: &str = "手机号 电话 联系方式 phone";

const ID_CARD_LEN: usize = 18;
const ID_CARD_WEIGHTS: [u32; 17] = [7, 9, 10, 5, 8, 4, 2, 1, 6, 3, 7, 9, 10, 5, 8, 4, 2];
const ID_CARD_CHECK_CODES: [char; 11] = ['1', '0', 'X', '9', '8', '7', '6', '5', '4', '3', '2'];

/// 文档正文中识别到的 PII 类型集合。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct PiiTypes {
    pub identity_card: bool,
    pub phone: bool,
}

impl PiiTypes {
    fn is_empty(self) -> bool {
        !self.identity_card && !self.phone
    }

    fn keywords(self) -> Vec<&'static str> {
        let mut out = Vec::new();
        if self.identity_card {
            out.push(IDENTITY_CARD_KEYWORDS);
        }
        if self.phone {
            out.push(PHONE_KEYWORDS);
        }
        out
    }
}

/// 从正文里识别受控的 PII 类型。
pub(crate) fn detect_pii_types(text: &str) -> PiiTypes {
    PiiTypes {
        identity_card: contains_valid_identity_card(text),
        phone: contains_mainland_phone(text),
    }
}

/// 给 FTS body 追加 PII 类型关键词。隐私约束：只追加类型词，不复制号码明文。
pub(crate) fn append_pii_keywords_for_fts(body: &str) -> String {
    let pii = detect_pii_types(body);
    if pii.is_empty() {
        return body.to_string();
    }
    let keywords = pii.keywords().join(" ");
    if body.is_empty() {
        keywords
    } else {
        format!("{body}\n{keywords}")
    }
}

fn contains_valid_identity_card(text: &str) -> bool {
    for (start, _) in text.char_indices() {
        let candidate: String = text[start..]
            .chars()
            .take_while(char::is_ascii_alphanumeric)
            .take(ID_CARD_LEN)
            .collect();
        if candidate.len() != ID_CARD_LEN {
            continue;
        }
        let end = start + candidate.len();
        if has_ascii_alnum_before(text, start) || has_ascii_alnum_after(text, end) {
            continue;
        }
        if is_valid_identity_card(&candidate) {
            return true;
        }
    }
    false
}

fn is_valid_identity_card(candidate: &str) -> bool {
    let bytes = candidate.as_bytes();
    if bytes.len() != ID_CARD_LEN {
        return false;
    }
    if !bytes[..17].iter().all(u8::is_ascii_digit) {
        return false;
    }
    let Some(check) = candidate.chars().last().map(|c| c.to_ascii_uppercase()) else {
        return false;
    };
    if !(check.is_ascii_digit() || check == 'X') {
        return false;
    }
    let sum: u32 = bytes[..17]
        .iter()
        .zip(ID_CARD_WEIGHTS)
        .map(|(b, weight)| u32::from(b - b'0') * weight)
        .sum();
    ID_CARD_CHECK_CODES[usize::try_from(sum % 11).unwrap_or(0)] == check
}

fn contains_mainland_phone(text: &str) -> bool {
    for token in text.split(|c: char| !c.is_ascii_alphanumeric()) {
        if token.len() == 11 && is_mainland_phone(token) {
            return true;
        }
    }
    false
}

fn is_mainland_phone(token: &str) -> bool {
    let bytes = token.as_bytes();
    bytes.len() == 11
        && bytes[0] == b'1'
        && (b'3'..=b'9').contains(&bytes[1])
        && bytes.iter().all(u8::is_ascii_digit)
}

fn has_ascii_alnum_before(text: &str, index: usize) -> bool {
    text[..index]
        .chars()
        .next_back()
        .is_some_and(|c| c.is_ascii_alphanumeric())
}

fn has_ascii_alnum_after(text: &str, index: usize) -> bool {
    text[index..]
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphanumeric())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synth_identity_card(prefix17: &str) -> String {
        assert_eq!(prefix17.len(), 17);
        assert!(prefix17.bytes().all(|b| b.is_ascii_digit()));
        let sum: u32 = prefix17
            .bytes()
            .zip(ID_CARD_WEIGHTS)
            .map(|(b, weight)| u32::from(b - b'0') * weight)
            .sum();
        let check = ID_CARD_CHECK_CODES[usize::try_from(sum % 11).unwrap()];
        format!("{prefix17}{check}")
    }

    #[test]
    fn detects_valid_identity_card_and_rejects_bad_checksum() {
        let card = synth_identity_card("11010519900101123");
        let detected = detect_pii_types(&format!("报名信息 {card}"));
        assert!(detected.identity_card);

        let mut bad = card;
        bad.replace_range(17..18, "0");
        if is_valid_identity_card(&bad) {
            bad.replace_range(17..18, "1");
        }
        let detected = detect_pii_types(&format!("报名信息 {bad}"));
        assert!(!detected.identity_card);
    }

    #[test]
    fn detects_mainland_phone() {
        let detected = detect_pii_types("联系人手机号 13912345678");
        assert!(detected.phone);
    }

    #[test]
    fn append_keywords_never_copies_detected_numbers() {
        let card = synth_identity_card("11010519900101123");
        let phone = "13912345678";
        let body = format!("报名信息 {card} 联系 {phone}");
        let injected = append_pii_keywords_for_fts(&body);
        let suffix = injected.strip_prefix(&body).unwrap_or("");
        assert!(suffix.contains("身份证"));
        assert!(suffix.contains("手机号"));
        assert!(!suffix.contains(&card));
        assert!(!suffix.contains(phone));
    }
}
