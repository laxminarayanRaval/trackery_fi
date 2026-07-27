# trackery_fi — next-session handoff (2026-07-27)

Sprint 1 is code-complete and pushed on `worktree-sprint1-continue` (12 task commits +
icon/npx fix). The v0.1.0 release exists with the **Windows exe zip attached and working
path proven**. One open item:

## Open: APK job fails on the v0.1.0 tag

- Run: https://github.com/laxminarayanRaval/trackery_fi/actions/runs/30254938016 —
  `apk` job dies in "build APK" after ~156s. `windows-exe` succeeds on the same commit,
  so it's Android-specific (suspects: NDK env for openssl-src cross-compile, gradle,
  or the 0s `tauri android init` step silently doing nothing).
- CI logs are auth-walled: no `gh auth` (user refused gh login CLI), Chrome extension
  not connected. Either ask the user to paste the "build APK" log tail, or finish the
  Docker repro that was running when this session ended:
  `thyrlian/android-sdk` container + volumes `trackery-android-sdk:/opt/android-sdk`,
  `trackery-android-home:/root`, mount the worktree at /w, sdkmanager NDK 27.1.12297006
  + rustup + node 22 + `npx tauri android init` + `npx tauri android build --apk --debug
  --target aarch64`. (First NDK download corrupted once; the SDK volume now caches it.)
- After fixing: commit, push, delete + re-push tag v0.1.0, verify the release gains
  `trackery_fi-v0.1.0.apk`. Consider committing `src-tauri/gen/android/` if init is the culprit.

## Context that saves time

- Rust verification runs in Docker (user's standing preference — see memory):
  `docker run --rm -v <worktree>:/w -v trackery-cargo-registry:/usr/local/cargo/registry -v trackery-rustup:/usr/local/rustup -v trackery-target:/ct -w /w -e CARGO_TARGET_DIR=/ct rust:1 sh -c "cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace"`
  — currently fully green (42 tests).
- src-tauri checks need webkit deps in the container (apt-get libwebkit2gtk-4.1-dev
  libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev).
- Local Windows exe builds work via `npx tauri build --no-bundle --target
  x86_64-pc-windows-msvc` with `C:\Strawberry\perl\bin` on PATH (MSVC Build Tools installed).
- No PR was opened (user said commits suffice). pdfium test lib: `target/pdfium/`.
- One known flake: pdfium SIGABRT (`free(): invalid pointer`) in lib tests under heavy
  parallel load — rerun, it's not deterministic.

## After the APK is green

Phone acceptance per ACCEPTANCE.md with real corpus statements, record parse accuracy,
then merge `worktree-sprint1-continue` → main (CI runs on main push).
