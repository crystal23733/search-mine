use super::*;
const NOW: i64 = 1_800_000_000;
#[derive(Clone)]
struct Bits(Vec<bool>);
impl Bits {
    fn new() -> Self {
        Self(Vec::new())
    }
    fn put(&mut self, value: u64, n: usize) {
        for i in (0..n).rev() {
            self.0.push(value & (1 << i) != 0);
        }
    }
    fn flags(&mut self, ids: &[u16], n: usize) {
        for id in 1..=n {
            self.put(u64::from(ids.contains(&(id as u16))), 1);
        }
    }
    fn finish(mut self) -> String {
        while !self.0.len().is_multiple_of(6) {
            self.0.push(false);
        }
        let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
        self.0
            .chunks(6)
            .map(|group| alphabet[group.iter().fold(0, |n, &b| n * 2 + usize::from(b))] as char)
            .collect()
    }
}
fn core(google: bool, range: bool, restriction: Option<u64>) -> Bits {
    let mut b = Bits::new();
    b.put(2, 6);
    b.put(((NOW - 10) * 10) as u64, 36);
    b.put(((NOW - 1) * 10) as u64, 36);
    b.put(u64::from(GOOGLE_CMP_ID), 12);
    b.put(1, 12);
    b.put(0, 6);
    b.put(0, 12);
    b.put(185, 12);
    b.put(5, 6);
    b.put(1, 1);
    b.put(0, 1);
    b.put(0, 12);
    b.flags(&[1, 3, 4], 24);
    b.put(0, 24);
    b.put(0, 1);
    b.put(0, 12);
    b.put(755, 16);
    b.put(u64::from(range), 1);
    if range {
        b.put(u64::from(google), 12);
        if google {
            b.put(1, 1);
            b.put(1, 16);
            b.put(755, 16);
        }
    } else {
        b.flags(if google { &[755] } else { &[] }, 755);
    }
    b.put(0, 16);
    b.put(0, 1);
    b.put(u64::from(restriction.is_some()), 12);
    if let Some(kind) = restriction {
        b.put(3, 6);
        b.put(kind, 2);
        b.put(1, 12);
        b.put(0, 1);
        b.put(755, 16);
    }
    b
}
fn publisher(ids: &[u16], custom: u64) -> Bits {
    let mut b = Bits::new();
    b.put(3, 3);
    b.flags(ids, 24);
    b.put(0, 24);
    b.put(custom, 6);
    for _ in 0..custom * 2 {
        b.put(0, 1);
    }
    b
}
fn proof(google: bool, range: bool, restriction: Option<u64>, ids: &[u16]) -> String {
    format!(
        "{}.{}",
        core(google, range, restriction).finish(),
        publisher(ids, 0).finish()
    )
}
fn flip(raw: &str, start: usize, value: u64, n: usize) -> String {
    let mut b = Bits::new();
    for byte in raw.bytes() {
        let index = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_"
            .iter()
            .position(|&c| c == byte)
            .unwrap();
        b.put(index as u64, 6);
    }
    for i in 0..n {
        b.0[start + i] = value & (1 << (n - i - 1)) != 0;
    }
    b.finish()
}
#[test]
fn google_tcf_separates_publisher_collection_from_google_ad_consent() {
    for range in [false, true] {
        assert_eq!(
            verify_tcf(&proof(true, range, None, &[1, 7, 8, 10]), NOW),
            Ok((true, true))
        );
        assert_eq!(
            verify_tcf(&proof(true, range, None, &[1, 8, 10]), NOW),
            Ok((true, false))
        );
        assert_eq!(
            verify_tcf(&proof(true, range, None, &[1, 7]), NOW),
            Ok((false, true))
        );
        assert_eq!(
            verify_tcf(&proof(false, range, None, &[1, 7, 8, 10]), NOW),
            Ok((true, false))
        );
        assert_eq!(
            verify_tcf(&core(true, range, None).finish(), NOW),
            Ok((false, false))
        );
    }
    for restriction in [0, 2] {
        assert_eq!(
            verify_tcf(&proof(true, true, Some(restriction), &[1, 7, 8, 10]), NOW),
            Ok((true, false))
        );
    }
    assert_eq!(
        verify_tcf(&proof(true, true, Some(1), &[1, 7, 8, 10]), NOW),
        Ok((true, true))
    );
    assert!(verify_tcf(&proof(true, true, Some(3), &[1, 7, 8, 10]), NOW).is_err());
}
#[test]
fn google_tcf_rejects_untrusted_versions_times_provider_and_noncanonical_input() {
    let c = core(true, true, None).finish();
    let pubpart = publisher(&[1, 7, 8, 10], 0).finish();
    for (offset, value, n) in [
        (0, 1, 6),
        (78, 301, 12),
        (132, 63, 6),
        (138, 0, 1),
        (6, ((NOW + 1) * 10) as u64, 36),
        (42, ((NOW + 1) * 10) as u64, 36),
    ] {
        assert!(
            verify_tcf(&format!("{}.{}", flip(&c, offset, value, n), pubpart), NOW).is_err(),
            "offset {offset}"
        );
    }
    for raw in [
        "".to_owned(),
        "not.tcf".into(),
        format!("{c}.{pubpart}.{pubpart}"),
        format!("{c}.{pubpart}.!"),
        format!("{c}.{pubpart}A"),
        "A".repeat(MAX_TCF_CHARS + 1),
    ] {
        assert!(verify_tcf(&raw, NOW).is_err());
    }
    assert!(verify_tcf(&proof(true, true, None, &[1, 7]), -1).is_err());
}
#[test]
fn google_tcf_checks_custom_fields_and_optional_vendor_segments_without_expanding_ranges() {
    let c = core(true, true, None).finish();
    for custom in [0, 1, 63] {
        let raw = format!("{}.{}", c, publisher(&[1, 7, 8, 10], custom).finish());
        assert_eq!(verify_tcf(&raw, NOW), Ok((true, true)));
    }
    for kind in [1, 2] {
        let mut segment = Bits::new();
        segment.put(kind, 3);
        segment.put(65535, 16);
        segment.put(1, 1);
        segment.put(1, 12);
        segment.put(1, 1);
        segment.put(1, 16);
        segment.put(65535, 16);
        let raw = format!(
            "{}.{}.{}",
            c,
            segment.finish(),
            publisher(&[1, 7, 8, 10], 0).finish()
        );
        assert_eq!(verify_tcf(&raw, NOW), Ok((true, true)));
    }
}

#[test]
fn google_tcf_every_truncated_prefix_and_invalid_structural_range_is_closed() {
    let full = proof(true, true, None, &[1, 7, 8, 10]);
    for end in 0..full.len() {
        assert_ne!(
            verify_tcf(&full[..end], NOW),
            Ok((true, true)),
            "prefix {end}"
        );
    }
    let c = core(true, true, None).finish();
    let p = publisher(&[1, 7, 8, 10], 0).finish();
    for (offset, value, n) in [
        (108, 63, 6),
        (120, 0, 12),
        (201, 63, 6),
        (230, 4095, 12),
        (243, 0, 16),
        (259, 756, 16),
        (259, 0, 16),
        (292, 4095, 12),
    ] {
        assert!(
            verify_tcf(&format!("{}.{}", flip(&c, offset, value, n), p), NOW).is_err(),
            "offset {offset}"
        );
    }
    let c = core(true, true, Some(0)).finish();
    for (offset, value, n) in [(304, 0, 6), (310, 3, 2), (324, 1, 1), (325, 0, 16)] {
        assert!(verify_tcf(&format!("{}.{}", flip(&c, offset, value, n), p), NOW).is_err());
    }
    assert!(verify_tcf(&format!("{c}A.{p}"), NOW).is_err());
    let mut unknown = Bits::new();
    unknown.put(4, 3);
    assert!(verify_tcf(&format!("{c}.{}", unknown.finish()), NOW).is_err());
}
#[tokio::test]
async fn google_tcf_current_user_verifier_is_explicit_configured_bounded_and_not_remote_auth() {
    use super::super::ConsentVerifier;
    let subject = uuid::Uuid::new_v4();
    let evidence = ConsentEvidence {
        provider: ConsentEvidenceProvider::GoogleTcfV2,
        tc_string: proof(true, true, None, &[1, 7, 8, 10]),
    };
    for provider in [None, Some("")] {
        let v = GoogleConsentVerifier::from_provider(provider).unwrap();
        assert_eq!(
            v.verify_submitted(subject, NOW, Some(&evidence)).await,
            Err(ConsentError::Unavailable)
        );
    }
    assert!(GoogleConsentVerifier::from_provider(Some("unknown")).is_err());
    let v = GoogleConsentVerifier::from_provider(Some("google")).unwrap();
    assert_eq!(v.verify(subject, NOW).await, Err(ConsentError::Unavailable));
    assert_eq!(
        v.verify_submitted(subject, NOW, None).await,
        Err(ConsentError::Unavailable)
    );
    let result = v
        .verify_submitted(subject, NOW, Some(&evidence))
        .await
        .unwrap();
    assert_eq!(result.subject, subject);
    assert!(result.analytics && result.advertising);
    assert_eq!(result.expires_at, NOW + 3600);
    assert_eq!(
        v.verify_submitted(subject, crate::events::MAX_EVENT_TIMESTAMP, Some(&evidence))
            .await,
        Err(ConsentError::Invalid)
    );
    let invalid = ConsentEvidence {
        provider: ConsentEvidenceProvider::GoogleTcfV2,
        tc_string: "bad".into(),
    };
    assert_eq!(
        v.verify_submitted(subject, NOW, Some(&invalid)).await,
        Err(ConsentError::Invalid)
    );
}
#[test]
fn google_tcf_shared_fixture_matches_the_protocol_and_publisher_permission() {
    let value: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../tests/fixtures/google-consent.json"
    ))
    .unwrap();
    for (name, expected) in [
        ("allowed", (true, true)),
        ("denied", (false, false)),
        ("analytics", (true, false)),
    ] {
        assert_eq!(
            verify_tcf(value[name]["tcString"].as_str().unwrap(), NOW),
            Ok(expected)
        );
    }
}
