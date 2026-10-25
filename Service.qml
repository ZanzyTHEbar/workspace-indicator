import QtQuick
import Quickshell.Io

// Quattro owns this child process for the lifetime of the enabled plugin.
// Install the binary first; do not also enable the systemd unit.
Item {
    Process {
        command: ["sh", "-c", "command -v workspace-indicator >/dev/null && workspace-indicator --help | grep -q -- --plugin-managed"]
        running: true
        stderr: StdioCollector {
            onStreamFinished: if (this.text.length > 0) console.error(this.text)
        }
        onExited: (exitCode, exitStatus) => {
            if (exitCode === 0) {
                indicator.running = true
            } else {
                console.error("workspace-indicator: install/update the separate native binary with --plugin-managed support on the shell's PATH first (see README Omarchy quick start), then disable and re-enable zanzythebar.workspace-indicator to retry.")
            }
        }
    }
    Process {
        id: indicator
        command: ["workspace-indicator", "--plugin-managed"]
        stderr: StdioCollector {
            onStreamFinished: if (this.text.length > 0) console.error(this.text)
        }
        onExited: (exitCode, exitStatus) => {
            console.warn("workspace-indicator exited (code " + exitCode + ", status " + exitStatus + "). Check the shell log and binary installation; disable and re-enable zanzythebar.workspace-indicator to retry.")
        }
    }
}
