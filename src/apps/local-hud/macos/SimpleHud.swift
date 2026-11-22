import AppKit
import Darwin

// Menu-bar Simple HUD for the modified local-only build.
// Before it reads the text file it applies a seatbelt: no network, no writes,
// read only the file's directory and system libraries. A failure does not read.
// This process does not load URLs, open a web view, start a child, or use Apple events.
//
// GitHub-hosted macOS CI can compile and launch this binary. That is not a test
// on the user's Mac mini.

@_silgen_name("sandbox_init")
func sandboxInit(
    _ profile: UnsafePointer<CChar>,
    _ flags: UInt64,
    _ errorbuf: UnsafeMutablePointer<UnsafeMutablePointer<CChar>?>?
) -> Int32

let arguments = CommandLine.arguments
let selfTest = arguments.count == 3 && arguments[1] == "--self-test"
guard arguments.count == 2 || selfTest else {
    fputs("usage: SimpleHud <hud.txt>\n       SimpleHud --self-test <hud.txt>\n", stderr)
    exit(2)
}
let hudPath = selfTest ? arguments[2] : arguments[1]

func refuse(_ message: String) -> Never {
    fputs("SimpleHud: \(message)\n", stderr)
    exit(2)
}

let hudURL = URL(fileURLWithPath: hudPath).resolvingSymlinksInPath()
let directory = hudURL.deletingLastPathComponent().path
let allowed = CharacterSet.alphanumerics.union(CharacterSet(charactersIn: "/._-+ "))
if directory.unicodeScalars.contains(where: { !allowed.contains($0) }) {
    refuse("refusing a path the sandbox profile cannot quote")
}
let profile = """
(version 1)
(deny default)
(deny network*)
(deny network-inbound)
(deny network-outbound)
(deny network-bind)
(deny file-write*)
(allow file-read* (subpath "/usr/lib") (subpath "/System") (subpath "/usr/share") (subpath "/Library"))
(allow file-read* (subpath "\(directory)"))
(allow signal (target self))
(allow sysctl-read)
(allow mach-lookup
    (global-name "com.apple.system.logger")
    (global-name "com.apple.CoreServices.coreservicesd"))
"""
let sandboxCode = profile.withCString { pointer in
    sandboxInit(pointer, 0, nil)
}
if sandboxCode != 0 {
    refuse("sandbox_init failed (\(sandboxCode)); the HUD text was not read")
}

let text: String
do {
    text = try String(contentsOf: hudURL, encoding: .utf8)
} catch {
    refuse("could not read the HUD text file")
}

final class Delegate: NSObject, NSApplicationDelegate {
    let text: String
    let selfTest: Bool
    var item: NSStatusItem?
    var panel: NSPanel?

    init(text: String, selfTest: Bool) {
        self.text = text
        self.selfTest = selfTest
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        let app = NSApplication.shared
        app.setActivationPolicy(.accessory)
        let status = app.statusBar.statusItem(withLength: NSStatusItem.variableLength)
        let first = text.split(separator: "\n", maxSplits: 1, omittingEmptySubsequences: false).first
        let title = String((first ?? "Agent HUD Local").prefix(120))
        status.button?.title = title
        status.button?.target = self
        status.button?.action = #selector(togglePanel)
        item = status
        let built = makePanel()
        panel = built
        if selfTest {
            let hasItem = item?.button != nil
            let hasPanel = built.contentView != nil
            print("simple_hud_self_test status_item=\(hasItem ? 1 : 0) panel=\(hasPanel ? 1 : 0)")
            if !hasItem || !hasPanel || !text.contains("Agent HUD Local") {
                exit(1)
            }
            app.terminate(nil)
        }
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
let delegate = Delegate(text: text, selfTest: selfTest)
retainedDelegate = delegate
app.delegate = delegate
app.run()
