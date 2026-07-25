# trackery_fi

**Your bank statement never leaves your phone. Works in airplane mode.**

A privacy-first, local-only personal finance tracker for India. Parses password-protected
bank statement PDFs (Bank of Baroda, HDFC, ICICI) fully on-device and stores transactions
in an SQLCipher-encrypted database.

## Install on Android (sideload from Releases)

1. Open this repo's **Releases** page and download the `.apk` from the latest release
   (releases are built automatically from `v*` tags).
2. On the phone, open the downloaded APK. When prompted, allow your browser/file manager
   to **install unknown apps** (Settings → Apps → Special app access → Install unknown apps).
3. Confirm the install. Debug-signed builds may show a Play Protect warning — choose
   "Install anyway".
4. Updates: install the newer APK over the old one (same signature required — a
   debug-signed build won't update a release-signed install; uninstall first).
