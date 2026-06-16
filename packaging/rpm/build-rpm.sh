#!/usr/bin/env bash
#
# Build a local, installable .rpm of Web App Hub.
#
# Requires: cargo, rpmbuild (dnf install rpm-build rpmdevtools)
# Output:   target/<name>-<version>-1.<dist>.<arch>.rpm
#
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

NAME=web-app-hub
ASSETS=assets/desktop
VERSION="$(grep -m1 '^version' Cargo.toml | sed -E 's/.*"([^"]+)".*/\1/')"

echo "==> Building $NAME $VERSION (release)"
cargo build --release -p app

echo "==> Staging package sources"
STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT
PKGDIR="$STAGE/$NAME-$VERSION"
mkdir -p "$PKGDIR"
install -m0755 target/release/web-app-hub                      "$PKGDIR/"
install -m0644 "$ASSETS/org.pvermeer.WebAppHub.desktop"        "$PKGDIR/"
install -m0644 "$ASSETS/org.pvermeer.WebAppHub.metainfo.xml"   "$PKGDIR/"
install -m0644 "$ASSETS/org.pvermeer.WebAppHub.png"            "$PKGDIR/"
install -m0644 LICENSE                                         "$PKGDIR/"

RPMTOP="$ROOT/target/rpmbuild"
mkdir -p "$RPMTOP"/{BUILD,RPMS,SOURCES,SPECS,SRPMS}
tar -C "$STAGE" -czf "$RPMTOP/SOURCES/$NAME-$VERSION.tar.gz" "$NAME-$VERSION"

echo "==> Running rpmbuild"
rpmbuild \
    --define "_topdir $RPMTOP" \
    --define "appver $VERSION" \
    -bb "$ROOT/packaging/rpm/$NAME.spec"

RPM="$(find "$RPMTOP/RPMS" -name '*.rpm' -print -quit)"
cp "$RPM" "$ROOT/target/"
echo
echo "==> Done: target/$(basename "$RPM")"
echo "    Install with: sudo dnf install ./target/$(basename "$RPM")"
