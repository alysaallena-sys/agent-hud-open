import Darwin
import Foundation

// Sandbox negative probe. Not the HUD. It tries a read outside the opted-in
// directory, a write, and a TCP connect. The seatbelt must deny all three.
// 203.0.113.1 is the documentation range. The socket times out in 2 seconds
// if the deny does not fire, so a failed sandbox cannot hang the job.

let arguments = CommandLine.arguments
guard arguments.count == 3 else {
    fputs("usage: SandboxProbe <inside-dir> <outside-file>\n", stderr)
    exit(2)
}
let inside = arguments[1]
let outside = arguments[2]

let outsideRead = (try? String(contentsOfFile: outside, encoding: .utf8)) == nil ? "denied" : "allowed"
let note = (inside as NSString).appendingPathComponent("note.txt")
let insideRead = (try? String(contentsOfFile: note, encoding: .utf8)) == nil ? "denied" : "ok"
let writePath = (inside as NSString).appendingPathComponent("probe-write")
let writeResult: String
do {
    try Data("no".utf8).write(to: URL(fileURLWithPath: writePath))
    writeResult = "allowed"
} catch {
    writeResult = "denied"
}

var connectResult = "denied"
let fd = socket(AF_INET, SOCK_STREAM, 0)
if fd >= 0 {
    var timeout = timeval(tv_sec: 2, tv_usec: 0)
    _ = setsockopt(
        fd,
        SOL_SOCKET,
        SO_SNDTIMEO,
        &timeout,
        socklen_t(MemoryLayout<timeval>.size)
    )
    var address = sockaddr_in()
    address.sin_family = sa_family_t(AF_INET)
    address.sin_port = UInt16(9).bigEndian
    address.sin_addr.s_addr = inet_addr("203.0.113.1")
    let result = withUnsafePointer(to: &address) { pointer in
        pointer.withMemoryRebound(to: sockaddr.self, capacity: 1) { socketAddress in
            connect(fd, socketAddress, socklen_t(MemoryLayout<sockaddr_in>.size))
        }
    }
    if result == 0 {
        connectResult = "allowed"
    }
    close(fd)
}

print("outside_read=\(outsideRead)")
print("inside_read=\(insideRead)")
print("write=\(writeResult)")
print("connect=\(connectResult)")
if outsideRead == "denied" && insideRead == "ok" && writeResult == "denied" && connectResult == "denied" {
    exit(0)
}
exit(1)
