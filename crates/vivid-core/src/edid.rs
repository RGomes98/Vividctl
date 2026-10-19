//! A minimal EDID reader: just what identifies a monitor (ADR-0003).
//!
//! Only the 128-byte base block is read. Extension blocks are ignored. The input is untrusted
//! bytes from a monitor, so nothing here panics or indexes without a bounds check.

use std::str;

use crate::{EdidError, MonitorSpec, SerialQuality};

/// Numeric serials that factory-set many unrelated models. Measured on 92,054 real EDIDs
/// (`docs/spikes/sp-05-output-identity/README.md`): a value shared by 50 or more different models.
pub const FACTORY_DEFAULT_SERIALS: [u32; 5] = [0, 1, 0x0100_0e00, 0x0101_0101, 0x5445];

const HEADER: [u8; 8] = [0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00];
const BASE_BLOCK: usize = 128;
const DESCRIPTOR_OFFSETS: [usize; 4] = [54, 72, 90, 108];
const DESCRIPTOR_LEN: usize = 18;
const TAG_MONITOR_NAME: u8 = 0xFC;
const TAG_SERIAL_NUMBER: u8 = 0xFF;

/// The identifying fields of an EDID base block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Edid {
    vendor:        String,
    product_code:  u16,
    serial_number: u32,
    week:          u8,
    year:          u16,
    product_name:  Option<String>,
    serial_text:   Option<String>,
    checksum_ok:   bool,
}

impl Edid {
    /// Reads the base block.
    ///
    /// The block checksum is recorded ([`Edid::checksum_ok`]) but does not make parsing fail:
    /// monitors with a wrong checksum still have a usable identity.
    ///
    /// # Errors
    ///
    /// [`EdidError::TooShort`] when there are fewer than 128 bytes, [`EdidError::BadHeader`] when
    /// the fixed 8-byte header is missing.
    pub fn parse(bytes: &[u8]) -> Result<Self, EdidError> {
        let block: &[u8] = bytes
            .get(..BASE_BLOCK)
            .ok_or(EdidError::TooShort { len: bytes.len() })?;
        if !block.starts_with(&HEADER) {
            return Err(EdidError::BadHeader);
        }
        let byte = |index: usize| block.get(index).copied().unwrap_or(0);

        let manufacturer: u16 = u16::from_be_bytes([byte(8), byte(9)]);
        // Three 5-bit letters, 1 = 'A'. Out-of-range values map to the next ASCII characters, the
        // same way Mutter decodes them.
        let letter = |shift: u16| char::from(u8::try_from((manufacturer >> shift) & 0x1F).unwrap_or(0) + b'@');

        let mut product_name: Option<String> = None;
        let mut serial_text: Option<String> = None;
        for offset in DESCRIPTOR_OFFSETS {
            let Some(descriptor) = block.get(offset..offset + DESCRIPTOR_LEN) else {
                continue;
            };
            // A display descriptor starts with three zero bytes, then its tag.
            if descriptor.get(..3) != Some(&[0, 0, 0]) {
                continue;
            }
            match descriptor.get(3).copied() {
                Some(TAG_MONITOR_NAME) => product_name = descriptor_text(descriptor),
                Some(TAG_SERIAL_NUMBER) => serial_text = descriptor_text(descriptor),
                _ => {}
            }
        }

        Ok(Self {
            vendor: [letter(10), letter(5), letter(0)].iter().collect(),
            product_code: u16::from_le_bytes([byte(10), byte(11)]),
            serial_number: u32::from_le_bytes([byte(12), byte(13), byte(14), byte(15)]),
            week: byte(16),
            year: 1990 + u16::from(byte(17)),
            product_name,
            serial_text,
            checksum_ok: block.iter().fold(0_u8, |sum, &b| sum.wrapping_add(b)) == 0,
        })
    }

    /// The 3-letter manufacturer code.
    #[must_use]
    pub fn vendor(&self) -> &str {
        &self.vendor
    }

    /// The numeric product code.
    #[must_use]
    pub const fn product_code(&self) -> u16 {
        self.product_code
    }

    /// The numeric serial (often a factory default, see [`FACTORY_DEFAULT_SERIALS`]).
    #[must_use]
    pub const fn serial_number(&self) -> u32 {
        self.serial_number
    }

    /// The manufacture week (0 when unspecified) and year.
    #[must_use]
    pub const fn manufactured(&self) -> (u8, u16) {
        (self.week, self.year)
    }

    /// The monitor name descriptor, if present and valid text.
    #[must_use]
    pub fn product_name(&self) -> Option<&str> {
        self.product_name.as_deref()
    }

    /// The serial number descriptor, if present and valid text.
    #[must_use]
    pub fn serial_text(&self) -> Option<&str> {
        self.serial_text.as_deref()
    }

    /// Whether the 128 bytes of the base block add up to zero modulo 256.
    #[must_use]
    pub const fn checksum_ok(&self) -> bool {
        self.checksum_ok
    }

    /// The monitor spec, by Mutter's rule: the name descriptor or `0x%04x` of the product code, and
    /// the serial descriptor or `0x%08x` of the numeric serial.
    #[must_use]
    pub fn spec(&self) -> MonitorSpec {
        MonitorSpec::new(
            self.vendor.clone(),
            self.product_name
                .clone()
                .unwrap_or_else(|| format!("0x{:04x}", self.product_code)),
            self.serial_text
                .clone()
                .unwrap_or_else(|| format!("0x{:08x}", self.serial_number)),
        )
    }

    /// How well this EDID can tell two units of the same model apart.
    #[must_use]
    pub fn serial_quality(&self) -> SerialQuality {
        if self.serial_text.is_some() || !FACTORY_DEFAULT_SERIALS.contains(&self.serial_number) {
            SerialQuality::Usable
        } else {
            SerialQuality::Sentinel
        }
    }
}

/// The 13 text bytes of a display descriptor, up to the first newline, without trailing spaces.
/// Text that is not valid UTF-8 counts as absent, as in Mutter.
fn descriptor_text(descriptor: &[u8]) -> Option<String> {
    let text: &[u8] = descriptor.get(5..DESCRIPTOR_LEN)?;
    let end: usize = text.iter().position(|&b| b == b'\n').unwrap_or(text.len());
    let value: &str = str::from_utf8(text.get(..end)?).ok()?.trim_end();
    (!value.is_empty()).then(|| value.to_owned())
}

#[cfg(test)]
// Tests build and corrupt fixed-size fixtures by index, and expect valid input.
#[allow(clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use super::*;

    /// A valid 128-byte base block with the given identity, for tests.
    struct Builder {
        bytes: Vec<u8>,
    }

    impl Builder {
        fn new(vendor: &str, product: u16, serial: u32) -> Self {
            let mut bytes: Vec<u8> = vec![0; BASE_BLOCK];
            bytes[..8].copy_from_slice(&HEADER);
            let letters: Vec<u16> = vendor.bytes().map(|c| u16::from(c - b'A' + 1)).collect();
            let id: u16 = (letters[0] << 10) | (letters[1] << 5) | letters[2];
            bytes[8..10].copy_from_slice(&id.to_be_bytes());
            bytes[10..12].copy_from_slice(&product.to_le_bytes());
            bytes[12..16].copy_from_slice(&serial.to_le_bytes());
            bytes[16] = 36;
            bytes[17] = 30;
            Self { bytes }
        }

        fn descriptor(mut self, slot: usize, tag: u8, text: &[u8]) -> Self {
            let offset: usize = DESCRIPTOR_OFFSETS[slot];
            self.bytes[offset + 3] = tag;
            let mut padded: Vec<u8> = text.to_vec();
            if padded.len() < 13 {
                padded.push(b'\n');
            }
            padded.resize(13, b' ');
            self.bytes[offset + 5..offset + 18].copy_from_slice(&padded);
            self
        }

        fn name(self, name: &str) -> Self {
            self.descriptor(1, TAG_MONITOR_NAME, name.as_bytes())
        }

        fn serial(self, serial: &str) -> Self {
            self.descriptor(2, TAG_SERIAL_NUMBER, serial.as_bytes())
        }

        fn build(mut self) -> Vec<u8> {
            let sum: u8 = self.bytes[..127].iter().fold(0, |a, &b| a.wrapping_add(b));
            self.bytes[127] = 0_u8.wrapping_sub(sum);
            self.bytes
        }
    }

    fn parse(bytes: &[u8]) -> Edid {
        Edid::parse(bytes).expect("valid test EDID")
    }

    #[test]
    fn reads_the_fixed_fields() {
        let edid: Edid = parse(&Builder::new("PHL", 0xc219, 0x8fac).build());
        assert_eq!(edid.vendor(), "PHL");
        assert_eq!(edid.product_code(), 0xc219);
        assert_eq!(edid.serial_number(), 0x8fac);
        assert_eq!(edid.manufactured(), (36, 2020));
        assert!(edid.checksum_ok());
    }

    #[test]
    fn spec_uses_descriptors_when_present() {
        let edid: Edid = parse(
            &Builder::new("PHL", 0x1234, 0x8fac)
                .name("PHL 242V8")
                .serial("ABC123")
                .build(),
        );
        assert_eq!(edid.spec(), MonitorSpec::new("PHL", "PHL 242V8", "ABC123"));
    }

    #[test]
    fn spec_falls_back_to_numeric_fields_like_mutter() {
        let edid: Edid = parse(&Builder::new("PHL", 0x1234, 0x8fac).build());
        assert_eq!(edid.spec(), MonitorSpec::new("PHL", "0x1234", "0x00008fac"));
    }

    #[test]
    fn the_name_without_a_serial_descriptor_uses_the_numeric_serial() {
        let edid: Edid = parse(&Builder::new("PHL", 0xc219, 0x8fac).name("PHL 242V8").build());
        assert_eq!(edid.spec(), MonitorSpec::new("PHL", "PHL 242V8", "0x00008fac"));
    }

    #[test]
    fn descriptor_text_stops_at_the_newline_and_drops_trailing_spaces() {
        let mut block: Vec<u8> = Builder::new("DEL", 1, 2).build();
        let offset: usize = DESCRIPTOR_OFFSETS[1];
        block[offset + 3] = TAG_MONITOR_NAME;
        block[offset + 5..offset + 18].copy_from_slice(b"U2720Q  \nJUNK");
        assert_eq!(parse(&block).product_name(), Some("U2720Q"));
    }

    #[test]
    fn a_full_width_descriptor_without_a_newline_is_read_whole() {
        let edid: Edid = parse(&Builder::new("DEL", 1, 2).name("ABCDEFGHIJKLM").build());
        assert_eq!(edid.product_name(), Some("ABCDEFGHIJKLM"));
    }

    #[test]
    fn an_empty_or_blank_descriptor_counts_as_absent() {
        let edid: Edid = parse(&Builder::new("DEL", 0x42, 7).name("   ").serial("").build());
        assert_eq!(edid.product_name(), None);
        assert_eq!(edid.serial_text(), None);
        assert_eq!(edid.spec(), MonitorSpec::new("DEL", "0x0042", "0x00000007"));
    }

    #[test]
    fn descriptor_text_that_is_not_utf8_counts_as_absent() {
        let edid: Edid = parse(
            &Builder::new("DEL", 0x42, 7)
                .descriptor(1, TAG_MONITOR_NAME, &[0xFF, 0xFE, 0xFD])
                .build(),
        );
        assert_eq!(edid.product_name(), None);
    }

    #[test]
    fn other_descriptor_tags_are_ignored() {
        let edid: Edid = parse(
            &Builder::new("DEL", 0x42, 7)
                .descriptor(0, 0xFD, b"range limits")
                .build(),
        );
        assert_eq!(edid.product_name(), None);
        assert_eq!(edid.serial_text(), None);
    }

    #[test]
    fn a_detailed_timing_block_is_not_mistaken_for_a_descriptor() {
        // A timing block has a non-zero pixel clock, so it does not start with three zero bytes.
        let mut block: Vec<u8> = Builder::new("DEL", 1, 2).build();
        let offset: usize = DESCRIPTOR_OFFSETS[1];
        block[offset] = 0x02;
        block[offset + 1] = 0x3A;
        block[offset + 3] = TAG_MONITOR_NAME;
        assert_eq!(parse(&block).product_name(), None);
    }

    #[test]
    fn serial_quality_flags_factory_defaults() {
        let usable: Edid = parse(&Builder::new("DEL", 1, 0x1234_5678).build());
        let described: Edid = parse(&Builder::new("DEL", 1, 0).serial("SN1").build());
        assert_eq!(usable.serial_quality(), SerialQuality::Usable);
        assert_eq!(described.serial_quality(), SerialQuality::Usable);
        for serial in FACTORY_DEFAULT_SERIALS {
            assert_eq!(
                parse(&Builder::new("DEL", 1, serial).build()).serial_quality(),
                SerialQuality::Sentinel
            );
        }
    }

    #[test]
    fn a_wrong_checksum_is_recorded_not_fatal() {
        let mut block: Vec<u8> = Builder::new("DEL", 1, 2).build();
        block[127] = block[127].wrapping_add(1);
        let edid: Edid = parse(&block);
        assert!(!edid.checksum_ok());
        assert_eq!(edid.vendor(), "DEL");
    }

    #[test]
    fn extension_blocks_after_the_base_block_are_ignored() {
        let mut bytes: Vec<u8> = Builder::new("PHL", 1, 2).name("EXT").build();
        bytes.extend_from_slice(&[0xAB; 128]);
        assert_eq!(parse(&bytes).product_name(), Some("EXT"));
    }

    #[test]
    fn rejects_short_input() {
        assert_eq!(Edid::parse(&[]), Err(EdidError::TooShort { len: 0 }));
        assert_eq!(Edid::parse(&[0; 127]), Err(EdidError::TooShort { len: 127 }));
    }

    #[test]
    fn rejects_a_missing_header() {
        assert_eq!(Edid::parse(&[0; 128]), Err(EdidError::BadHeader));
        let mut block: Vec<u8> = Builder::new("DEL", 1, 2).build();
        block[0] = 0x01;
        assert_eq!(Edid::parse(&block), Err(EdidError::BadHeader));
    }

    #[test]
    fn never_panics_on_any_single_byte_corruption() {
        let good: Vec<u8> = Builder::new("PHL", 0xc219, 0x8fac)
            .name("PHL 242V8")
            .serial("SN")
            .build();
        for index in 0..good.len() {
            for value in [0x00, 0x0A, 0x7F, 0x80, 0xFF] {
                let mut block: Vec<u8> = good.clone();
                block[index] = value;
                let _ = Edid::parse(&block).map(|edid| (edid.spec(), edid.serial_quality()));
            }
        }
    }
}
