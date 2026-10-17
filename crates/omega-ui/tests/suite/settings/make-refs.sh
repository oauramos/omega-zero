#!/bin/bash
# Screenshots flow.html's Settings into ref/, for quick_menu.rs to hold what Rust composes to:
# the modal alone, cropped from the screen at modal_rect() and taken back to one pixel per art
# pixel (each art pixel is a 2x2 block on the screen; the script checks every block is one
# colour). Run it again after changing Settings in assets/ui/flow.html or its components.
#
#   crates/omega-ui/tests/suite/settings/make-refs.sh
#
# Each ref is named settings-<canvas>-<colour>-<row>.png, <row> being the row flow.html's
# #row= puts the cursor on (0 Fast Forward, 6 Date & Time). Needs python3 with Pillow.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
flow="$(cd "$here/../../../../../assets/ui" && pwd)/flow.html"
chrome="${CHROME:-/Applications/Google Chrome.app/Contents/MacOS/Google Chrome}"
mkdir -p "$here/ref"
shot="$(mktemp -d)"
trap 'rm -rf "$shot"' EXIT

# canvas, colour, row
shoot() {
  local w="${1%x*}" h="${1#*x}" name="settings-$1-$2-$3"
  "$chrome" --headless=new --disable-gpu --hide-scrollbars --force-device-scale-factor=1 \
    --virtual-time-budget=3000 --window-size="$w,$h" --screenshot="$shot/$name.png" \
    "file://$flow#screen=settings&size=$1&colour=$2&row=$3&shot=1" 2>/dev/null
  # modal_rect(), in art pixels: 16 in at the sides, 14 from the top, 5 above the 15 pixel bar
  python3 -I - "$shot/$name.png" "$here/ref/$name.png" "$w" "$h" <<'EOF'
import sys
from PIL import Image
src, out, w, h = sys.argv[1], sys.argv[2], int(sys.argv[3]), int(sys.argv[4])
x, y = 16, 14
mw, mh = w // 2 - 2 * 16, h // 2 - 15 - 14 - 5
im = Image.open(src).convert('RGB')
art = Image.new('RGB', (mw, mh))
for ay in range(mh):
    for ax in range(mw):
        block = {im.getpixel((2 * (x + ax) + dx, 2 * (y + ay) + dy)) for dx in (0, 1) for dy in (0, 1)}
        if len(block) != 1:
            sys.exit(f'{src}: art pixel ({ax}, {ay}) is not one colour: {block}')
        art.putpixel((ax, ay), block.pop())
art.save(out)
EOF
}

for canvas in 640x480 720x480 720x720; do
  shoot "$canvas" night 0
done
shoot 640x480 amber 6
shoot 720x480 amber 6
