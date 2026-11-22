#include <arpa/inet.h>
#include <errno.h>
#include <fcntl.h>
#include <netinet/in.h>
#include <stdio.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/time.h>
#include <unistd.h>

/* Sandbox negative probe. Not the HUD. It uses only the system C library so a
   seatbelt that denies the Swift runtime can still run it. 203.0.113.1 is the
   documentation range. connect times out in 2 seconds if the deny does not fire. */

static int read_ok(const char *path) {
    char buffer[8];
    int fd = open(path, O_RDONLY);
    if (fd < 0) {
        return 0;
    }
    ssize_t n = read(fd, buffer, sizeof buffer);
    close(fd);
    return n >= 0;
}

int main(int argc, char **argv) {
    char note[4096];
    char probe_write[4096];
    int outside_ok;
    int inside_ok;
    int wrote;
    int connected = 0;
    int connect_errno = 0;
    int fd;
    struct timeval timeout;
    struct sockaddr_in address;

    if (argc != 3) {
        fprintf(stderr, "usage: SandboxProbe <inside-dir> <outside-file>\n");
        return 2;
    }
    if (snprintf(note, sizeof note, "%s/note.txt", argv[1]) >= (int)sizeof note) {
        return 2;
    }
    if (snprintf(probe_write, sizeof probe_write, "%s/probe-write", argv[1]) >= (int)sizeof probe_write) {
        return 2;
    }

    outside_ok = read_ok(argv[2]);
    inside_ok = read_ok(note);
    fd = open(probe_write, O_WRONLY | O_CREAT | O_TRUNC, 0600);
    wrote = fd >= 0;
    if (fd >= 0) {
        close(fd);
    }

    fd = socket(AF_INET, SOCK_STREAM, 0);
    if (fd < 0) {
        connect_errno = errno;
    } else {
        timeout.tv_sec = 2;
        timeout.tv_usec = 0;
        setsockopt(fd, SOL_SOCKET, SO_SNDTIMEO, &timeout, sizeof timeout);
        memset(&address, 0, sizeof address);
        address.sin_family = AF_INET;
        address.sin_port = htons(9);
        address.sin_addr.s_addr = inet_addr("203.0.113.1");
        if (connect(fd, (struct sockaddr *)&address, sizeof address) == 0) {
            connected = 1;
        } else {
            connect_errno = errno;
        }
        close(fd);
    }

    printf("outside_read=%s\n", outside_ok ? "allowed" : "denied");
    printf("inside_read=%s\n", inside_ok ? "ok" : "denied");
    printf("write=%s\n", wrote ? "allowed" : "denied");
    printf("connect=%s\n", connected ? "allowed" : "denied");
    printf("connect_errno=%d\n", connect_errno);
    if (!outside_ok && inside_ok && !wrote && !connected) {
        return 0;
    }
    return 1;
}
