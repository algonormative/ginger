# Packaging

Target: crates.io `lemon-ginger`, binary `ginger` (the bare crate name is taken by an unrelated package). Native release assets bundle the trusted SynthDefs, schemas and their hashes. Until a release exists, install from a checkout:

```bash
cargo install --path crates/lemon-ginger --locked
```

The Lemon Audio Studio Kit (`uv tool install 'music-rig[studio]'`, then `music-rig setup studio`) obtains a compatible Ginger release; versions are independent per repo with a kit compatibility manifest.
