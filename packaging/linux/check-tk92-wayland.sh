#!/bin/bash
# Run inside bbport-tk92(-python), forwarding only a native Wayland socket.
set -euo pipefail
: "${XDG_RUNTIME_DIR:?Mount a Wayland socket and set XDG_RUNTIME_DIR}"
: "${WAYLAND_DISPLAY:?Set WAYLAND_DISPLAY to the forwarded socket name}"
test -S "$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY"
export LD_LIBRARY_PATH=/opt/tk92/lib:/opt/bb-deps/lib
unset DISPLAY
if test "$#" -gt 0; then
    timeout 90 "$@"
else
    cat > /tmp/bb-tk92-test.tcl <<'TCL'
puts "TK_VERSION=[package require Tk]"
puts "WINDOW_SYSTEM=[tk windowingsystem]"
puts "DISPLAY_EXISTS=[info exists env(DISPLAY)]"
if {[tk windowingsystem] ne "wayland"} {error "Native Wayland backend required"}
wm title . "Bloodborne native Tk Wayland test"
pack [ttk::label .label -text "Tk 9.2 / Wayland / Debian 11 / Clang"] -padx 40 -pady 30
pack [ttk::button .button -text Close -command {destroy .}] -pady 20
after 1000 {destroy .}
TCL
    timeout 20 /opt/tk92/bin/wish9.2 /tmp/bb-tk92-test.tcl
fi
