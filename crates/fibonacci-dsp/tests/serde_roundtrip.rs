//! Preset round-trip: everything a saved sound is made of must survive
//! JSON serialization exactly. Runs only with `--features serde`.
#![cfg(feature = "serde")]

use fibonacci_dsp::{MelodyParams, Patch, RatioMode, VerbParams, ALGORITHMS};

fn round_trip<T: serde::Serialize + for<'a> serde::Deserialize<'a> + std::fmt::Debug>(value: &T) {
    let json = serde_json::to_string_pretty(value).expect("serialize");
    let back: T = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(format!("{value:?}"), format!("{back:?}"), "lossy round trip");
}

#[test]
fn params_survive_json_round_trip() {
    for &alg in &ALGORITHMS {
        let mut patch = Patch::init(alg, RatioMode::GoldenMirror);
        patch.rip = 0.7;
        patch.index = 0.33;
        patch.ops[2].enabled = false;
        patch.ops[4].detune_cents = -7.5;
        round_trip(&patch);
    }
    round_trip(&VerbParams::default());
    round_trip(&MelodyParams::default());
}
