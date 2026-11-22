import AppKit

// Simple HUD and menu bar for the modified local-only Agent HUD build.
// Displays a text file produced by agent-hud-local. This file was not
// compiled in the environment that added it.
//
// The shell does not load URLs, open a web view, start a child process,
// send Apple events, or touch the keychain.

let arguments = CommandLine.arguments
guard arguments.count == 2 else {
    fputs("usage: SimpleHud <hud.txt>\n", stderr)
    exit(2)
}

let text: String
do {
    text = try String(contentsOfFile: arguments[1], encoding: .utf8)
} catch {
    fputs("SimpleHud: could not read the HUD text file\n", stderr)
    exit(2)
}

final class Delegate: NSObject, NSApplicationDelegate {
    let text: String
    var item: NSStatusItem?
    var panel: NSPanel?

    init(text: String) {
        self.text = text
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        let app = NSApplication.shared
        app.setActivationPolicy(.accessory)
        let status = app.statusBar.statusItem(withLength: NSStatusItem.variableLength)
        let first = text.split(separator: "\n", maxSplits: 1, omittingEmptySubsequences: false).first
        status.button?.title = String((first ?? "Agent HUD Local").prefix(120))
        status.button?.target = self
        status.button?.action = #selector(togglePanel)
        item = status
        panel = makePanel()
    }

    func makePanel() -> NSPanel {
        let visible = NSScreen.main?.visibleFrame ?? NSRect(x: 0, y: 0, width: 800, height: 600)
        let width: CGFloat = 460
        let height: CGFloat = 320
        let frame = NSRect(
            x: visible.midX - width / 2,
            y: visible.maxY - height - 8,
            width: width,
            height: height
        )
        let panel = NSPanel(
            contentRect: frame,
            styleMask: [.titled, .closable, .nonactivatingPanel],
            backing: .buffered,
            defer: false
        )
        panel.level = .floating
        panel.isFloatingPanel = true
        panel.hidesOnDeactivate = false
        panel.title = "Agent HUD Local"
        panel.isReleasedWhenClosed = false
        let field = NSTextField(wrappingLabelWithString: text)
        field.frame = NSRect(x: 12, y: 12, width: width - 24, height: height - 40)
        field.isEditable = false
        field.isSelectable = true
        panel.contentView?.addSubview(field)
        return panel
    }

    @objc func togglePanel() {
        guard let panel else { return }
        if panel.isVisible {
            panel.orderOut(nil)
        } else {
            panel.orderFrontRegardless()
        }
    }
}

private var retainedDelegate: Delegate?

let app = NSApplication.shared
let delegate = Delegate(text: text)
retainedDelegate = delegate
app.delegate = delegate
app.run()
