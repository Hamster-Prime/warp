// Ensure cargo recompiles the i18n crate when locale YAML files change.
// Without this, modifying _locales/*.yml won't trigger a rebuild and
// rust-i18n's compile-time-embedded translations will be stale.
fn main() {
    println!("cargo:rerun-if-changed=_locales");
}
