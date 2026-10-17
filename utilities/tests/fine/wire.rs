//! The wire: values drawn from the seed, sent and received, come back
//! as they went; words cut short or naming no value are refused.

use utilities::rng::RandomSource;
use utilities::wire::{receive, send, WireReader, Wired, WiredStruct};

/// A struct of every kind of field the wire knows, a table of handles.
#[derive(Clone, Debug, Default, PartialEq)]
struct EveryKind {
    /// A word.
    word: u64,
    /// A narrower number.
    narrow: u16,
    /// A flag.
    flag: bool,
    /// A float, NaN and all, as its bits.
    float: f64,
    /// Text, of any length and any characters.
    text: String,
    /// A list.
    list: Vec<u64>,
    /// Maybe a pair.
    maybe_pair: Option<(u32, i64)>,
    /// A list of the struct's own kind of thing: lists of lists.
    nested: Vec<Option<String>>,
}

impl WiredStruct for EveryKind {
    const FIELDS: &'static [fn(&mut Self) -> &mut dyn Wired] = &[|value| &mut value.word, |value| &mut value.narrow, |value| &mut value.flag, |value| &mut value.float, |value| &mut value.text, |value| &mut value.list, |value| &mut value.maybe_pair, |value| &mut value.nested];
}

/// Text of `length` characters drawn from `random`, some far past ASCII.
fn drawn_text(random: &mut RandomSource, length: u64) -> String {
    (0..length).map(|_| char::from_u32(random.draw_below(0x11_0000) as u32).unwrap_or('é')).collect()
}

/// One drawn from `random`.
fn drawn(random: &mut RandomSource) -> EveryKind {
    let text_length = random.draw_below(40);
    EveryKind {
        word: random.next_draw(),
        narrow: random.next_draw() as u16,
        flag: random.percent_chance(50),
        float: f64::from_bits(random.next_draw()),
        text: drawn_text(random, text_length),
        list: (0..random.draw_below(20)).map(|_| random.next_draw()).collect(),
        maybe_pair: random.percent_chance(50).then(|| (random.next_draw() as u32, random.next_draw() as i64)),
        nested: (0..random.draw_below(5)).map(|_| random.percent_chance(50).then(|| drawn_text(random, 9))).collect(),
    }
}

/// Values sent one after another down one stream come back the same,
/// in order, and the stream ends after the last.
#[test]
fn what_is_sent_comes_back_the_same() {
    let mut random = RandomSource::new(crate::test_seed::TestSeedHandle::open().fine_seed());
    let mut sent: Vec<EveryKind> = (0..200).map(|_| drawn(&mut random)).collect();
    let mut stream = Vec::new();
    for value in &mut sent {
        send(value, &mut stream).expect("sent to memory");
    }
    let mut reader = stream.as_slice();
    for value in &sent {
        let received: EveryKind = receive(&mut reader).expect("read").expect("a value");
        // NaNs compare unequal: the float is compared by its bits.
        assert_eq!((received.float.to_bits(), EveryKind { float: 0.0, ..received.clone() }), (value.float.to_bits(), EveryKind { float: 0.0, ..value.clone() }));
    }
    assert!(receive::<EveryKind>(&mut reader).expect("read").is_none(), "nothing after the last");
}

/// A value's words cut short anywhere are refused, never taken as some
/// other value.
#[test]
fn words_cut_short_are_refused() {
    let mut random = RandomSource::new(crate::test_seed::TestSeedHandle::open().fine_seed());
    for _ in 0..50 {
        let mut words = Vec::new();
        drawn(&mut random).put(&mut words);
        let cut_at = random.draw_below(words.len() as u64) as usize;
        let mut taken = EveryKind::default();
        assert!(taken.take(&mut WireReader::new(&words[..cut_at])).is_none(), "{cut_at} of {} words taken", words.len());
    }
}

/// A flag of neither 0 nor 1, and text that is no UTF-8, name no value.
#[test]
fn words_naming_no_value_are_refused() {
    let mut flag = false;
    assert!(flag.take(&mut WireReader::new(&[2])).is_none());
    let mut text = String::new();
    assert!(text.take(&mut WireReader::new(&[1, 0xff])).is_none());
}
