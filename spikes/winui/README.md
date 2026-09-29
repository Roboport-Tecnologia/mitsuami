# WinUI spike (M0.5)

A throwaway spike that drives WinUI 3 imperatively through our own
`windows-bindgen` 0.100 bindings. It is not a workspace member. The findings are
in `docs/ARCHITECTURE.md` §15 (WinUI) and §18.

It needs Windows App Runtime 2.4 or later installed, and the MSVC toolchain:

```
cargo +1.96-x86_64-pc-windows-msvc run                   # scripted checks, report, exit
cargo +1.96-x86_64-pc-windows-msvc run -- --interactive  # leave the window open
```

`build.rs` downloads the WinAppSDK metadata from NuGet into `OUT_DIR` and
generates the bindings listed in `filter.txt`. The scripted run writes a
capture to `%TEMP%\mitsuami-winui-spike.png`.
