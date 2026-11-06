xattr 0.2.3 (MIT/Apache-2.0) vendored with one change in src/sys/mod.rs: ENOATTR is ENODATA on Linux/Android, because libc 0.2.191 no longer exports libc::ENOATTR there.
