
/// Streams of one seed are apart: no draw of one turns up early in
/// another, as it would were a stream the seed moved along.
#[test]
fn streams_of_a_seed_share_no_draws() {
    let mut seen = std::collections::HashSet::new();
    let seed = utilities::seed::counted();
    // Streams one after another from a first one drawn: as superchunks beside each other are.
    let first = utilities::rng::Rng::new(seed).draw();
    for stream in (0..64u64).map(|nth| first.wrapping_add(nth)) {
        let mut random = utilities::rng::Rng::for_stream(seed, stream);
        for _ in 0..1_000 {
            assert!(seen.insert(random.draw()), "a draw of stream {stream} seen before");
        }
    }
}
