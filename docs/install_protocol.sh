#!/usr/bin/env bash
# Register oculus:// callback handler so the browser login redirect lands in
# the launcher: run meta-pcvr-launcher --url "oculus://link/frl_login?token=..."
set -e
BIN="${1:-iris-launcher}"
mkdir -p "$HOME/.local/share/applications"
cat > "$HOME/.local/share/applications/iris-handler.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=Iris Launcher callback
Exec=$BIN --url "%u"
MimeType=x-scheme-handler/oculus
NoDisplay=true
EOF
xdg-desktop-menu install "$HOME/.local/share/applications/iris-handler.desktop" 2>/dev/null || true
echo "handler installed: $BIN --url handles oculus://"
