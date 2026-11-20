# ctext

Currently only support utf8 mode

License: MIT

## Vendored in Backplane

xim-ctext 0.3.0 from crates.io (https://github.com/Riey/xim-rs, `xim-ctext/` at 65f77e44524b, MIT), patched in through `native/gpui/Cargo.toml` `[patch.crates-io]`.
One change, in `src/lib.rs`: `compound_text_to_utf8` decodes COMPOUND_TEXT by its character sets. Upstream read any text that did not start with an escape
as UTF-8, but COMPOUND_TEXT's GR starts as ISO 8859-1's right half: ibus over XIM in a C locale sends the us-intl dead acute's preedit as the single byte
0xB4 (´) and the composed é as 0xE9, which upstream refused (and zed-xim's `expect` turned into a panic). Now: GL ASCII and GR Latin-1 by default,
`ESC ( F` / `ESC ) F` (ASCII, JIS X 0201 Roman and Katakana), `ESC - F` (ISO 8859 right halves), `ESC $ ( F` / `ESC $ ) F` (GB 2312, JIS X 0208, KS C 5601),
UTF-8 segments (`ESC % G` .. `ESC % @`, anywhere in the text) and direction sequences (skipped). Anything else is an error, never a panic
(upstream also panicked slicing a UTF-8 segment shorter than its end marker). Tests in `src/lib.rs`.
