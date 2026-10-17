//! The wire: values as words, to be sent -- between a client and a
//! server, say -- and taken back. A value puts its words after the others ([`Wired::put`]) and
//! is taken back in place ([`Wired::take`]); a struct is a table of
//! mutable handles to its fields ([`WiredStruct::FIELDS`]), each put
//! and taken in the table's order -- as a world's generation keeps its
//! numbers. Words, never a machine's bytes: the same on any machine.

/// Words being read: what values were put as, from the first.
pub struct WireReader<'words> {
    /// The words.
    words: &'words [u64],
    /// The next to read.
    at: usize,
}

impl<'words> WireReader<'words> {
    /// Reading `words` from the first.
    pub fn new(words: &'words [u64]) -> Self {
        Self { words, at: 0 }
    }

    /// The next word: none past the last.
    pub fn next_word(&mut self) -> Option<u64> {
        let word = *self.words.get(self.at)?;
        self.at += 1;
        Some(word)
    }

    /// The next `count` words: none if fewer are left.
    pub fn next_words(&mut self, count: usize) -> Option<&'words [u64]> {
        let taken = self.words.get(self.at..self.at.checked_add(count)?)?;
        self.at += count;
        Some(taken)
    }

    /// Whether every word was read.
    pub fn is_done(&self) -> bool {
        self.at == self.words.len()
    }
}

/// What goes on the wire.
pub trait Wired {
    /// Puts its words after `words`.
    fn put(&mut self, words: &mut Vec<u64>);

    /// Takes its value from `wire`, in place of its own: none if the
    /// words end early or name no such value.
    fn take(&mut self, wire: &mut WireReader) -> Option<()>;
}

/// A struct on the wire: its fields, each a handle to it, put and taken
/// in this order.
pub trait WiredStruct: 'static {
    /// Its fields.
    const FIELDS: &'static [fn(&mut Self) -> &mut dyn Wired];
}

impl<Struct: WiredStruct> Wired for Struct {
    fn put(&mut self, words: &mut Vec<u64>) {
        for field in Self::FIELDS {
            field(self).put(words);
        }
    }

    fn take(&mut self, wire: &mut WireReader) -> Option<()> {
        Self::FIELDS.iter().try_for_each(|field| field(self).take(wire))
    }
}

impl Wired for u64 {
    fn put(&mut self, words: &mut Vec<u64>) {
        words.push(*self);
    }

    fn take(&mut self, wire: &mut WireReader) -> Option<()> {
        *self = wire.next_word()?;
        Some(())
    }
}

impl Wired for u32 {
    fn put(&mut self, words: &mut Vec<u64>) {
        words.push(u64::from(*self));
    }

    fn take(&mut self, wire: &mut WireReader) -> Option<()> {
        *self = wire.next_word()?.try_into().ok()?;
        Some(())
    }
}

impl Wired for u16 {
    fn put(&mut self, words: &mut Vec<u64>) {
        words.push(u64::from(*self));
    }

    fn take(&mut self, wire: &mut WireReader) -> Option<()> {
        *self = wire.next_word()?.try_into().ok()?;
        Some(())
    }
}

impl Wired for usize {
    fn put(&mut self, words: &mut Vec<u64>) {
        words.push(*self as u64);
    }

    fn take(&mut self, wire: &mut WireReader) -> Option<()> {
        *self = wire.next_word()?.try_into().ok()?;
        Some(())
    }
}

impl Wired for i64 {
    fn put(&mut self, words: &mut Vec<u64>) {
        words.push(*self as u64);
    }

    fn take(&mut self, wire: &mut WireReader) -> Option<()> {
        *self = wire.next_word()? as i64;
        Some(())
    }
}

impl Wired for bool {
    fn put(&mut self, words: &mut Vec<u64>) {
        words.push(u64::from(*self));
    }

    fn take(&mut self, wire: &mut WireReader) -> Option<()> {
        *self = match wire.next_word()? {
            0 => false,
            1 => true,
            _ => return None,
        };
        Some(())
    }
}

impl Wired for f64 {
    fn put(&mut self, words: &mut Vec<u64>) {
        words.push(self.to_bits());
    }

    fn take(&mut self, wire: &mut WireReader) -> Option<()> {
        *self = f64::from_bits(wire.next_word()?);
        Some(())
    }
}

impl Wired for f32 {
    fn put(&mut self, words: &mut Vec<u64>) {
        words.push(u64::from(self.to_bits()));
    }

    fn take(&mut self, wire: &mut WireReader) -> Option<()> {
        *self = f32::from_bits(wire.next_word()?.try_into().ok()?);
        Some(())
    }
}

/// Text: its length in bytes, then its bytes eight a word, the lowest first.
impl Wired for String {
    fn put(&mut self, words: &mut Vec<u64>) {
        words.push(self.len() as u64);
        words.extend(self.as_bytes().chunks(8).map(|eight_bytes| eight_bytes.iter().rev().fold(0, |word, &byte| word << 8 | u64::from(byte))));
    }

    fn take(&mut self, wire: &mut WireReader) -> Option<()> {
        let byte_count = usize::try_from(wire.next_word()?).ok()?;
        let text_words = wire.next_words(byte_count.div_ceil(8))?;
        let bytes: Vec<u8> = text_words.iter().flat_map(|word| word.to_le_bytes()).take(byte_count).collect();
        *self = String::from_utf8(bytes).ok()?;
        Some(())
    }
}

/// A list: its length, then each item.
impl<Item: Wired + Default> Wired for Vec<Item> {
    fn put(&mut self, words: &mut Vec<u64>) {
        words.push(self.len() as u64);
        self.iter_mut().for_each(|item| item.put(words));
    }

    fn take(&mut self, wire: &mut WireReader) -> Option<()> {
        let item_count = usize::try_from(wire.next_word()?).ok()?;
        self.clear();
        for _ in 0..item_count {
            let mut item = Item::default();
            item.take(wire)?;
            self.push(item);
        }
        Some(())
    }
}

/// Maybe a value: 0 for none, 1 and then the value.
impl<Value: Wired + Default> Wired for Option<Value> {
    fn put(&mut self, words: &mut Vec<u64>) {
        match self {
            None => words.push(0),
            Some(value) => {
                words.push(1);
                value.put(words);
            }
        }
    }

    fn take(&mut self, wire: &mut WireReader) -> Option<()> {
        *self = match wire.next_word()? {
            0 => None,
            1 => {
                let mut value = Value::default();
                value.take(wire)?;
                Some(value)
            }
            _ => return None,
        };
        Some(())
    }
}

impl<First: Wired, Second: Wired> Wired for (First, Second) {
    fn put(&mut self, words: &mut Vec<u64>) {
        self.0.put(words);
        self.1.put(words);
    }

    fn take(&mut self, wire: &mut WireReader) -> Option<()> {
        self.0.take(wire)?;
        self.1.take(wire)
    }
}

impl<Item: Wired, const COUNT: usize> Wired for [Item; COUNT] {
    fn put(&mut self, words: &mut Vec<u64>) {
        self.iter_mut().for_each(|item| item.put(words));
    }

    fn take(&mut self, wire: &mut WireReader) -> Option<()> {
        self.iter_mut().try_for_each(|item| item.take(wire))
    }
}

/// Sends `value` down `writer`: how many words it is, then its words,
/// each a word's eight bytes, the lowest first.
pub fn send(value: &mut impl Wired, writer: &mut impl std::io::Write) -> std::io::Result<()> {
    let mut words = Vec::new();
    value.put(&mut words);
    let mut bytes = Vec::with_capacity((1 + words.len()) * 8);
    for word in std::iter::once(words.len() as u64).chain(words) {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    writer.write_all(&bytes)?;
    writer.flush()
}

/// The next value sent down `reader` ([`send`]): none if it ended
/// before one began.
pub fn receive<Value: Wired + Default>(reader: &mut impl std::io::Read) -> std::io::Result<Option<Value>> {
    let mut count_bytes = [0u8; 8];
    match reader.read_exact(&mut count_bytes) {
        Err(read_error) if read_error.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        read_result => read_result?,
    }
    let word_count = usize::try_from(u64::from_le_bytes(count_bytes)).map_err(|_| std::io::Error::from(std::io::ErrorKind::InvalidData))?;
    let mut bytes = vec![0u8; word_count.checked_mul(8).ok_or(std::io::ErrorKind::InvalidData)?];
    reader.read_exact(&mut bytes)?;
    let words: Vec<u64> = bytes.as_chunks::<8>().0.iter().map(|&eight_bytes| u64::from_le_bytes(eight_bytes)).collect();
    let (mut value, mut wire) = (Value::default(), WireReader::new(&words));
    match value.take(&mut wire) {
        Some(()) if wire.is_done() => Ok(Some(value)),
        _ => Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "words that are no such value")),
    }
}
