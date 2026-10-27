//! Security conformance cases for story:the-pull-request-and-the-store-carry-the-evidence: the
//! reference the store records for a `test_result` is `<path>#sha256=<hex>&result=..&revision=..
//! &producer=..`, each part percent-encoded (`%`, `#`, `&`, `=`, a space and every byte outside
//! printable ASCII), so no part can add a fragment or a key or change another field. A property
//! over a fixed-seed sample of parts drawn from the bytes that matter.

use takt::frontier::{decode_reference_part, reference_part, test_result_reference};

/// A small fixed-seed generator (xorshift64*), so every run draws the same sample.
struct Draw(u64);

impl Draw {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// A text of up to 24 characters drawn from separators, controls, plain ASCII, the
    /// characters a markdown link or a YAML line is made of, and non-ASCII.
    fn text(&mut self) -> String {
        const POOL: &[char] = &[
            '#', '&', '=', '%', ' ', '\t', '\n', '\r', '\u{0}', '\u{1b}', '\u{7f}', '\u{85}',
            '\u{2028}', '\u{202e}', '\u{feff}', 'a', 'Z', '0', '9', '/', '.', '-', '_', '[', ']',
            '(', ')', '`', ':', '?', '+', 'é', 'ß', '中', '🦀',
        ];
        let len = usize::try_from(self.next() % 25).expect("a small length");
        (0..len)
            .map(|_| POOL[usize::try_from(self.next() % POOL.len() as u64).expect("an index")])
            .collect()
    }
}

/// Every part round-trips and holds no separator and no byte outside printable ASCII; a whole
/// reference has one `#`, and its fragment is exactly the four keys in order, each decoding to
/// the part given; the path before `#` decodes to the path given.
#[test]
fn every_reference_part_is_percent_encoded_and_no_part_adds_a_fragment_or_a_key() {
    let mut draw = Draw(0x5eed_0001_d00d_beef);
    for _ in 0..20_000 {
        let parts: Vec<String> = (0..4).map(|_| draw.text()).collect();
        for part in &parts {
            let encoded = reference_part(part);
            assert!(
                encoded.bytes().all(
                    |byte| (0x21..=0x7e).contains(&byte) && !matches!(byte, b'#' | b'&' | b'=')
                ),
                "{part:?} encodes to {encoded:?}"
            );
            assert_eq!(
                decode_reference_part(&encoded).as_deref(),
                Some(part.as_str()),
                "{part:?} round-trips"
            );
        }
        let reference = test_result_reference(&parts[0], b"{}", &parts[1], &parts[2], &parts[3]);
        assert_eq!(reference.matches('#').count(), 1, "{reference}");
        let (path, fragment) = reference.split_once('#').expect("a fragment");
        assert_eq!(
            decode_reference_part(path).as_deref(),
            Some(parts[0].as_str())
        );
        let fields: Vec<(&str, &str)> = fragment
            .split('&')
            .map(|field| field.split_once('=').expect("key=value"))
            .collect();
        let keys: Vec<&str> = fields.iter().map(|(key, _)| *key).collect();
        assert_eq!(
            keys,
            ["sha256", "result", "revision", "producer"],
            "{reference}"
        );
        for (index, (_, value)) in fields.iter().enumerate().skip(1) {
            assert_eq!(
                decode_reference_part(value).as_deref(),
                Some(parts[index].as_str()),
                "{reference}"
            );
        }
    }
}
