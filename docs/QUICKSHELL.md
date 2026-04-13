# Quickshell Integration

crawl is designed to be the backend for a Quickshell QML shell.

---

## Consuming the SSE stream

```qml
// In your Quickshell root or a dedicated Service component
import Quickshell
import Quickshell.Io

pragma Singleton

Singleton {
    id: root

    property real cpuUsage: 0
    property real batteryPercent: 0
    property string batteryState: "unknown"
    property bool onAc: true
    property var notifications: []

    // Active media player
    property string mediaTitle: ""
    property string mediaArtist: ""
    property string mediaStatus: "stopped"

    Process {
        id: eventStream
        command: ["curl", "--no-buffer",
                  "--unix-socket", Quickshell.env("XDG_RUNTIME_DIR") + "/crawl.sock",
                  "http://localhost/events"]
        running: true

        stdout: SplitParser {
            onRead: (line) => {
                if (!line.startsWith("data: ")) return
                try {
                    const evt = JSON.parse(line.slice(6))
                    root.handleEvent(evt)
                } catch (_) {}
            }
        }
    }

    function handleEvent(evt) {
        switch (evt.domain) {
        case "sysmon":
            if (evt.data.event === "cpu_update")
                root.cpuUsage = evt.data.cpu.aggregate
            break
        case "power":
            if (evt.data.event === "battery_update") {
                root.batteryPercent = evt.data.status.percent
                root.batteryState   = evt.data.status.state
                root.onAc           = evt.data.status.on_ac
            }
            break
        case "notify":
            if (evt.data.event === "new")
                root.notifications.push(evt.data.notification)
            else if (evt.data.event === "closed")
                root.notifications = root.notifications
                    .filter(n => n.id !== evt.data.id)
            break
        case "media":
            if (evt.data.event === "track_changed") {
                root.mediaTitle  = evt.data.player.title  ?? ""
                root.mediaArtist = evt.data.player.artist ?? ""
                root.mediaStatus = evt.data.player.status
            }
            break
        }
    }

    // One-shot HTTP requests to the daemon
    function setBrightness(percent) {
        crawlRequest("POST", "/brightness/set", { value: percent })
    }
    function setVolume(percent) {
        crawlRequest("POST", "/audio/volume", { percent: percent })
    }
    function mediaNext() {
        crawlRequest("POST", "/media/next", {})
    }
    function dismissNotification(id) {
        crawlRequest("DELETE", "/notify/" + id, null)
    }

    function crawlRequest(method, path, body) {
        // TODO: wire up via Quickshell NetworkRequest or a Process curl call
        // NetworkRequest doesn't support Unix sockets natively yet;
        // use Process + curl as a bridge or the CrawlDesktopShell axum bridge crate.
    }
}
```

---

## Bar widget examples

```qml
// Battery widget reading from CrawlService
Text {
    text: {
        const pct = CrawlService.batteryPercent.toFixed(0)
        const icon = CrawlService.onAc ? "\uf084" : "\uf079"
        return icon + " " + pct + "%"
    }
    color: CrawlService.batteryPercent < 20 ? "#f38ba8" : "#cdd6f4"
}

// CPU widget
Text {
    text: "\uf61a " + CrawlService.cpuUsage.toFixed(1) + "%"
}

// Media widget
Row {
    Text { text: CrawlService.mediaArtist + " — " + CrawlService.mediaTitle }
    MouseArea {
        onClicked: CrawlService.mediaNext()
    }
}
```
