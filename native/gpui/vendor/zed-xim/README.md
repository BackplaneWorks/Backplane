# xim-rs

XIM protocol handler in Rust

## Server example

You can see xim server example in my [IME](https://github.com/Riey/kime/tree/develop/src/frontends/xim)

## Minimum Safe Rust Version

The current Minimum Safe Rust Version in **1.64**. The current **tentative** policy is that any change in the MSRV will be accompanied by a minor version bump.

## project structure

### xim

Binding with X client libraries

### xim-parser

Read/Write xim message generated from xim-gen

### xim-gen

xim protocol parser generator

## features

- [x] Parse messages
- [x] Basic protocol
- [ ] Extension protocol
- [x] AttributeBuilder

## binding for X client

### xlib

- [x] client
- [ ] server

### x11rb

- [x] client
- [x] server

## limitations

* Only native endian is supported
* Only support utf-8 and JIS X0208-1983 of CTEXT
* Auth, StrConvertion doesn't supported since they are not used in real world

## Vendored in Backplane

zed-xim 0.4.0-zed from crates.io (Zed's fork of https://github.com/Riey/xim-rs, git 471c10cdf97b, MIT), without `examples/`, `benches/`, `.github/`, `.vscode/`
and their `[[example]]` / `[[bench]]` targets and dev-dependencies, patched in through `native/gpui/Cargo.toml` `[patch.crates-io]`. One change, in `src/client.rs`: a
PreeditDraw, Commit or ResetIcReply whose COMPOUND_TEXT does not decode is dropped and logged once per kind (`decode_ctext`: the reason and the length,
never the text) instead of `expect("Encoding Error")` panicking the client. The handler still runs with "" (an empty preedit clears the marked text, an
empty commit ends the composition), and a synchronous commit is still answered. The decoding itself is fixed in the vendored
xim-ctext (`../xim-ctext/README.md`).
