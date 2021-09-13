# valve-sdk13-rng

This is a direct translation to Rust of the original Source 2013 SDK Uniform
Random Number Generator from Valve, ported to golang by
[@Step7750](https://github.com/Step7750/UniformRandom "UniformRandom repo")

Thanks Step7750 for taking the time to reverse engineer it, and taking the time
of how skins work outside of the game.


## Usage

```rust
use valve_sdk13_rng::UniformRandomStream;

let mut gen = UniformRandomStream::with_seed(72);
let res = gen.random_f64(0_f64, 1_f64);
assert_eq!(0.543_099_8, res);
```

## More in-depth links
[[PSA] How Paint Seed Actually Works
(Technical)](https://www.reddit.com/r/GlobalOffensiveTrade/comments/b7g538/psa_how_paint_seed_actually_works_technical/
"Go to Reddit")
