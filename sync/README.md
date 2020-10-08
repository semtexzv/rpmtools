## RpmSync 
- UNRELEASED: Waiting for [serde-yaml support](https://github.com/dtolnay/serde-yaml/issues/175)
- [Crates.io](https://crates.io/crates/rpmsync)
- [Documentation](https://docs.rs/rpmsync) 

Provides extremely simple downloader for RPM Repositories. Goal is to use this to build more complex tools for working 
with RPM repositiories. Implemented in a streaming manner as much as possible in order to keep memory use low even when 
working with large repositiories. 