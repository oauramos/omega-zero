#!/bin/sh
# Talks to a console on the home network over SSH, for task deploy:wifi and task log:wifi.
#
#   console.sh deploy BINARY [FILE...]   copy BINARY to System/omega-zero, and every FILE into
#                                        System/, then reboot into it and wait for it to start
#   console.sh log                       print omega-zero's log
#
# CONSOLE (default rg35xxsp.local, HOST:PORT for another port) and SD (default /mnt/sdcard) say
# where. CONSOLE_PASSWORD is root's password; when it is not set, it is read from
# .console-password at the root of the repo, which git ignores, and failing that it is root,
# BaseOS's own. omega-zero gives root six random digits the first time it runs on a card whose
# baseos.conf sets no ssh_password; the console shows them under Settings > Network, Y on SSH.
#
# CORES set (task deploy:wifi CORES=1) has deploy copy the device cores into System/ as well:
# vendor/<name>_libretro.so for each name in DEVICE_CORES, by default the five the bundle
# ships. task cores:device builds them.
set -eu

repo="$(cd "$(dirname "$0")/.." && pwd)"
cores="${DEVICE_CORES:-mgba gpsp snes9x mednafen_supafaust pcsx_rearmed mednafen_pce_fast fceumm genesis_plus_gx}"
console="${CONSOLE:-rg35xxsp.local}"
port=22
case "$console" in
*:*)
	port="${console##*:}"
	console="${console%:*}"
	;;
esac
host="root@$console"
sd="${SD:-/mnt/sdcard}"
password_file="$repo/.console-password"
if [ -n "${CONSOLE_PASSWORD:-}" ]; then
	password="$CONSOLE_PASSWORD"
elif [ -f "$password_file" ]; then
	# its first line, without the spaces or line ending an editor leaves round it
	password="$(sed -n '1{s/^[[:space:]]*//;s/[[:space:]]*$//;p;}' "$password_file")"
else
	password=root
fi

command -v sshpass > /dev/null || {
	echo "console.sh: needs sshpass (brew install sshpass)" >&2
	exit 1
}

ssh_once() {
	sshpass -p "$password" ssh -p "$port" -o ConnectTimeout=5 -o StrictHostKeyChecking=no \
		-o UserKnownHostsFile=/dev/null -o LogLevel=ERROR \
		-o PreferredAuthentications=password -o PubkeyAuthentication=no "$host" "$@"
}

# on [-i FILE] COMMAND runs COMMAND on the console, with FILE as its input. Dropbear now and
# then turns the first login away, so a failed connection (ssh's 255, sshpass's 5) is tried
# again, reading FILE from the start; the command's own failure is not.
on() {
	input=/dev/null
	if [ "$1" = "-i" ]; then
		input="$2"
		shift 2
	fi
	tries=0
	while :; do
		status=0
		ssh_once "$@" < "$input" || status=$?
		case "$status" in
		255 | 5) ;;
		*) return "$status" ;;
		esac
		tries=$((tries + 1))
		if [ "$tries" -ge 3 ]; then
			# sshpass says 5 when the password is turned away
			if [ "$status" -eq 5 ]; then
				echo "console.sh: $host turned the password away" >&2
				echo "console.sh: the password is on the console under Settings > Network, Y on SSH; put it in .console-password" >&2
			else
				echo "console.sh: could not reach $host" >&2
			fi
			return "$status"
		fi
		sleep 1
	done
}

# Copies a file to the console under a temporary name, checks it arrived whole and renames it
# into place, so a shell running the old copy of a script keeps reading that one.
put() {
	on -i "$1" "cat > '$2.new'"
	want="$(shasum -a 256 "$1" | cut -d' ' -f1)"
	got="$(on "sha256sum '$2.new'" | cut -d' ' -f1)"
	if [ "$want" != "$got" ]; then
		echo "console.sh: $2 arrived damaged ($got, sent $want)" >&2
		exit 1
	fi
	on "mv -f '$2.new' '$2'"
}

lines() {
	ssh_once "cat '$1' 2> /dev/null | wc -l" < /dev/null 2> /dev/null | tr -d ' ' || true
}

case "${1:-}" in
deploy)
	binary="$2"
	shift 2
	# Every core is checked before anything is copied, so a missing one leaves the console as
	# it was.
	if [ -n "${CORES:-}" ]; then
		for lib in $cores; do
			so="$repo/vendor/${lib}_libretro.so"
			if ! file "$so" 2> /dev/null | grep -q aarch64; then
				echo "console.sh: $so is missing or not built for the console; task cores:device builds it" >&2
				exit 1
			fi
		done
	fi
	status=0
	on "test -d '$sd/System'" || status=$?
	case "$status" in
	0) ;;
	1)
		echo "console.sh: no $sd/System on $host, set SD" >&2
		exit 1
		;;
	*) exit 1 ;;
	esac
	log="$sd/omega-zero-session.log"
	before="$(lines "$log")"
	put "$binary" "$sd/System/omega-zero"
	on "chmod +x '$sd/System/omega-zero'"
	for f in "$@"; do
		put "$f" "$sd/System/$(basename "$f")"
	done
	# A core omega-zero has open keeps running from the copy it opened: the new one is renamed
	# over it, not written into it.
	if [ -n "${CORES:-}" ]; then
		for lib in $cores; do
			put "$repo/vendor/${lib}_libretro.so" "$sd/System/${lib}_libretro.so"
		done
	fi
	on sync
	echo "copied, rebooting $host"
	# Wi-Fi let go of first, as omega-zero's own restart does: the RTL8821CS's driver hangs the
	# kernel's shutdown when it has just gone into its power saving, as it does after scanning
	# for networks. wpa_supplicant, wpa_cli and the DHCP client stop, each interface goes down,
	# and its device is unbound from the driver, so the kernel has nothing of it left to stop.
	on "(sleep 1; kill \$(pidof wpa_supplicant wpa_cli udhcpc) 2> /dev/null; sleep 1; kill -9 \$(pidof wpa_supplicant wpa_cli udhcpc) 2> /dev/null; for n in /sys/class/net/*/wireless; do n=\${n%/wireless}; d=\$(readlink -f \$n/device); r=\$(readlink -f \$n/device/driver); ifconfig \${n##*/} down; done; [ -n \"\$r\" ] && echo \${d##*/} > \$r/unbind; sleep 1; reboot) > /dev/null 2>&1 &"
	# The launcher writes a line to the session log as it starts omega-zero. A console that is
	# back but turns the password away three times running has had a new one given to root: the
	# first omega-zero to run on a card with no ssh_password does that as it starts.
	waited=0
	refused=0
	while [ "$waited" -lt 120 ]; do
		sleep 3
		waited=$((waited + 3))
		status=0
		out="$(ssh_once "cat '$log' 2> /dev/null | wc -l" < /dev/null 2> /dev/null)" || status=$?
		if [ "$status" -eq 5 ]; then
			refused=$((refused + 1))
			if [ "$refused" -ge 3 ]; then
				echo "console.sh: $host is back, but turned the password away: omega-zero gave root a new one as it started" >&2
				echo "console.sh: the password is on the console under Settings > Network, Y on SSH; put it in .console-password" >&2
				exit 1
			fi
			continue
		fi
		refused=0
		now="$(printf '%s' "$out" | tr -d ' ')"
		if [ "${now:-0}" -gt "${before:-0}" ]; then
			ssh_once "tail -n 1 '$log'" < /dev/null
			exit 0
		fi
	done
	echo "console.sh: $host did not start omega-zero within 120 s" >&2
	exit 1
	;;
log)
	on "cat '$sd/omega-zero.log'"
	;;
*)
	echo "usage: console.sh deploy BINARY [FILE...] | console.sh log" >&2
	exit 2
	;;
esac
