
/// Streams of one seed are apart: no draw of one turns up early in
/// another, as it would were a stream the seed moved along.
#[test]
fn streams_of_a_seed_share_no_draws() {
    let mut seen_draws = std::collections::HashSet::new();
    let seed = utilities::seed::counted_seed();
    // Streams one after another from a first one drawn: as superchunks beside each other are.
    let first_stream = utilities::rng::RandomSource::new(seed).next_draw();
    for stream in (0..64u64).map(|stream_offset| first_stream.wrapping_add(stream_offset)) {
        let mut stream_random = utilities::rng::RandomSource::for_stream(seed, stream);
        for _ in 0..1_000 {
            assert!(seen_draws.insert(stream_random.next_draw()), "a draw of stream {stream} seen before");
        }
    }
}
