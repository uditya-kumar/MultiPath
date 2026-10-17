# Releasing MultiPath for macOS (Apple silicon)

Releases go to **https://github.com/timowse/MultiPath/releases**. The app looks for updates at

    https://github.com/timowse/MultiPath/releases/latest/download/latest.json

That URL always points at the **latest published, non-prerelease** release. A draft or a prerelease is invisible to
the updater, so publish it as a normal release once everything is uploaded.

## v1.2.0: files in this folder

Built and signed on 2026-10-17 by `scripts/release-macos.sh` (commit on branch `macos-port`). Upload these four files
to a release with tag **`v1.2.0`**. The names must stay exactly as they are, because `latest.json` points at
`…/releases/download/v1.2.0/MultiPath_1.2.0_aarch64.app.tar.gz`.

| file | what it is | sha256 |
| --- | --- | --- |
| `MultiPath_1.2.0_aarch64.dmg` | the installer people download by hand | `bb72791ace6fa8274090a08df401a5fc1eda150e17e6c742db5b55e809c10104` |
| `MultiPath_1.2.0_aarch64.app.tar.gz` | what the in-app updater downloads | `719f3a91b08ebc5eb0f888af2844a381e76469b67c8d42bb786c07dc793f2ab0` |
| `MultiPath_1.2.0_aarch64.app.tar.gz.sig` | its updater signature (minisign, also inside `latest.json`) | `2b298101843127b6e56c98434e1256b9781af09b99f35656215a85e480cdaf85` |
| `latest.json` | the update feed: version, notes, date, `darwin-aarch64` URL and signature | `c14dcfba899b98b85daf5f58e561d42b49851a3e07a37e4675d8b7fa36d66110` |

`SHA256SUMS` holds the same checksums (`shasum -a 256 -c SHA256SUMS`). Uploading it is optional.

The signature's trusted comment is `version:1.2.0`. The app is configured with `requireSignedVersion`, so a feed that
announces another version with this archive is refused. The signing key's ID is `334DFF4341E59A68`, the same key as
`plugins.updater.pubkey` in `src-tauri/tauri.conf.json`.

## Publish v1.2.0 (manual, nothing here does it for you)

1. Check the files: `cd release-assets && shasum -a 256 -c SHA256SUMS`, then
   `cd ../src-tauri && cargo test -- --ignored release_assets` (verifies the archive against the public key and
   `latest.json`, the same way the app will, and that a changed byte fails).
2. Commit and push the source to `timowse/MultiPath` and create the tag `v1.2.0` on that commit.
3. On GitHub: **Releases → Draft a new release**, tag `v1.2.0`, title `MultiPath 1.2.0`, upload the four files above.
4. Leave "Set as a pre-release" **unticked** and publish.
5. Check the feed: `curl -sL https://github.com/timowse/MultiPath/releases/latest/download/latest.json` shows
   version `1.2.0`, and the `url` inside it downloads (HTTP 200 after redirects).

With the GitHub CLI, steps 3–4 are
`gh release create v1.2.0 --repo timowse/MultiPath --title "MultiPath 1.2.0" --notes-file <notes>
release-assets/MultiPath_1.2.0_aarch64.dmg release-assets/MultiPath_1.2.0_aarch64.app.tar.gz
release-assets/MultiPath_1.2.0_aarch64.app.tar.gz.sig release-assets/latest.json`.

1.2.0 is the first version with the updater. People on 1.1.0 have to install the DMG by hand once. After that, the
app offers every newer release.

## The next release (1.2.1 and later)

1. Bump the version in **all three** places: `package.json` (plus `npm install` to sync `package-lock.json`),
   `src-tauri/Cargo.toml` and `src-tauri/tauri.conf.json`. `cargo test` fails if they disagree (`versions_agree`).
2. Run the checks (CLAUDE.md, rule 2), then
   `scripts/release-macos.sh "One or two plain sentences: what's new"`. The notes appear in the popup's update prompt.
3. Upload the four new files to a release tagged `v<version>` and publish it as above. The previous release's
   `latest.json` stops being "latest" automatically.

The script refuses to build if the key is missing, isn't mode 0600, doesn't match the public key in
`tauri.conf.json`, or if the versions disagree.

## The signing key

- Private key: `~/.config/multipath/updater.key` (mode 0600, folder 0700). It was generated with an empty password, so
  the file permissions are what protect it. Never commit, print or upload it. `.gitignore` excludes `*.key`.
- Public key: `~/.config/multipath/updater.key.pub`, the same text as `plugins.updater.pubkey` in
  `src-tauri/tauri.conf.json`.
- **Back it up** somewhere safe and offline, such as an encrypted disk image or a password manager. If it's lost,
  installed copies can never accept another update; people would have to reinstall a build with a new public key by
  hand.
- If it leaks, anyone could sign updates that installed copies accept. Generate a new key
  (`npx tauri signer generate -w <path>`), put its public key in `tauri.conf.json`, publish a release signed with the
  **old** key that carries the new public key, and sign everything after that with the new key.
- To build from another machine or CI, give it the key as a secret: `TAURI_SIGNING_PRIVATE_KEY` (the key's text or
  the path to the file) and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` (empty). `tauri build` ignores
  `TAURI_SIGNING_PRIVATE_KEY_PATH`.

## What this release is not

- **Not notarised and only ad-hoc signed** (no Apple Developer ID). The first open needs right-click → Open, and
  Gatekeeper's `spctl` rejects it. In-app updates still work: the updater checks its own minisign signature, which is
  independent of Apple code signing. A Developer ID build would also need `APPLE_*` notarisation variables for
  `tauri build`.
- Apple silicon only: there is no Intel build, and `latest.json` lists only `darwin-aarch64`.
