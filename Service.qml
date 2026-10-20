import QtQuick
import Quickshell.Io

// Quattro owns this child process for the lifetime of the enabled plugin.
// Install the binary first; do not also enable the systemd unit.
Item {
    Process {
        command: ["workspace-indicator"]
        running: true
    }
}
