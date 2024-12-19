# Shared crates for the services

Some common crates for the shine project:
- shine-test
- shine-test-macros
- shine-macros
- shine-service
  
## shine-test

Automatically initializing logging and other handy features for the tests.

This crate was highly inspired by the [test-log](https://crates.io/crates/test-log) crate.

### Requirements

- rustls requires some other dependencies and it may result in `aws-lc-sys` compile errors
  - <https://medium.com/@rrnazario/rust-how-to-fix-failed-to-run-custom-build-command-for-aws-lc-sys-on-windows-c3bd2405ac6f>
  - https://github.com/rustls/rustls/issues/1913

## shine-service

The common features for all the server projects.

