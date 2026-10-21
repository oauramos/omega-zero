#!/bin/sh
set -eu

here="$(cd "$(dirname "$0")" && pwd)"

device_cflags="-mcpu=cortex-a53"
# Beetle PCE Fast is C++. libstdc++ and libgcc go into the core itself, so it never asks BaseOS
# for a libstdc++ as new as the one it was built with; its version script keeps them out of sight.
device_ldflags="-static-libstdc++ -static-libgcc"

usage() {
	echo "usage: $0 stamp COMMIT | build COMMIT TARBALL WORKDIR OUT" >&2
	exit 2
}

sha256() {
	if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1"; else shasum -a 256 "$1"; fi |
		cut -d' ' -f1
}

stamp() {
	echo "commit=$1"
	echo "source=https://github.com/libretro/beetle-pce-fast-libretro/archive/$1.tar.gz"
	echo "recipe=make platform=unix"
	echo "device_cflags=$device_cflags"
	echo "device_ldflags=$device_ldflags"
	for p in "$here"/*.patch; do
		[ -f "$p" ] || continue
		echo "patch=$(basename "$p") sha256:$(sha256 "$p")"
	done
}

# Applies the patches with git kept out of any repository. Left to look, git finds the
# checkout this runs in, and a worktree's checkout names its repository by a path the build
# container does not have.
apply_patches() {
	ceiling="$(cd "$1/.." && pwd)"
	for p in "$here"/*.patch; do
		[ -f "$p" ] || continue
		GIT_CEILING_DIRECTORIES="$ceiling" git -C "$1" apply -p1 "$p"
	done
}

# Builds for the device on an aarch64 linux box, or on an x86_64 linux box with CROSS set to an
# aarch64 toolchain's prefix (aarch64-linux-gnu-), and for this Mac on a Mac.
build() {
	commit="$1" tarball="$2" work="$3" out="$4"
	src="$work/beetle-pce-fast-libretro-$commit"

	rm -rf "$work"
	mkdir -p "$work"
	tar -xzf "$tarball" -C "$work"
	if [ ! -f "$src/Makefile" ]; then
		echo "$tarball does not hold beetle-pce-fast-libretro-$commit/Makefile" >&2
		exit 1
	fi

	apply_patches "$src"

	tools=""
	case "$(uname -s)-$(uname -m)" in
	Linux-aarch64)
		platform=unix
		lib=mednafen_pce_fast_libretro.so
		cflags="$device_cflags"
		ldflags="$device_ldflags"
		;;
	Linux-x86_64)
		if [ -z "${CROSS:-}" ]; then
			echo "set CROSS to an aarch64 toolchain's prefix to build Beetle PCE Fast here" >&2
			exit 1
		fi
		platform=unix
		lib=mednafen_pce_fast_libretro.so
		cflags="$device_cflags"
		ldflags="$device_ldflags"
		tools="CC=${CROSS}gcc CXX=${CROSS}g++ AR=${CROSS}ar"
		;;
	Darwin-*)
		platform=osx
		lib=mednafen_pce_fast_libretro.dylib
		cflags=""
		ldflags=""
		;;
	*)
		echo "build Beetle PCE Fast on an aarch64 linux box or a Mac, not $(uname -s)-$(uname -m)" >&2
		exit 1
		;;
	esac

	# The Makefile asks git for the version, which here would name omega-zero's commit.
	# shellcheck disable=SC2086
	CFLAGS="$cflags" CXXFLAGS="$cflags" LDFLAGS="$ldflags" make -C "$src" platform="$platform" \
		$tools GIT_VERSION="\" $(printf %s "$commit" | cut -c1-7)\"" \
		-j"$(getconf _NPROCESSORS_ONLN)"

	mkdir -p "$(dirname "$out")"
	cp "$src/$lib" "$out"
	stamp "$commit" >"$out.meta"
}

case "${1:-}" in
stamp)
	[ $# -eq 2 ] && [ -n "$2" ] || usage
	stamp "$2"
	;;
build)
	[ $# -eq 5 ] && [ -n "$2" ] && [ -n "$3" ] && [ -n "$4" ] && [ -n "$5" ] || usage
	build "$2" "$3" "$4" "$5"
	;;
*)
	usage
	;;
esac
