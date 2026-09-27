/* Test-only fsync barrier, loaded into one owned host on a private IBus bus. */
#define _GNU_SOURCE
#include <dlfcn.h>
#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <time.h>
#include <unistd.h>

int fsync(int fd) {
    int (*real_fsync)(int) = dlsym(RTLD_NEXT, "fsync");
    if (real_fsync == NULL) { errno = ENOSYS; return -1; }
    const char *root = getenv("XDG_RUNTIME_DIR");
    const char *marker = getenv("SUZAKU_NATIVE_SYNC_QA");
    const char *address = getenv("IBUS_ADDRESS");
    const char *prefix = "/tmp/suzaku-sync-qa.";
    const char *display = getenv("DISPLAY");
    const char *wayland = getenv("WAYLAND_DISPLAY");
    struct stat info;
    if (root == NULL || marker == NULL || strcmp(marker, "1") != 0 || address == NULL ||
        strncmp(root, prefix, strlen(prefix)) != 0 || strchr(root + strlen(prefix), '/') != NULL ||
        (display != NULL && *display) || (wayland != NULL && *wayland) ||
        lstat(root, &info) != 0 || !S_ISDIR(info.st_mode) || info.st_uid != getuid()) {
        return real_fsync(fd);
    }
    char expected[PATH_MAX], link[64], path[PATH_MAX];
    snprintf(expected, sizeof(expected), "unix:path=%s/ibus.sock", root);
    if (strcmp(address, expected) != 0) { return real_fsync(fd); }
    snprintf(link, sizeof(link), "/proc/self/fd/%d", fd);
    ssize_t length = readlink(link, path, sizeof(path) - 1);
    if (length < 0) { return real_fsync(fd); }
    path[length] = '\0';
    snprintf(expected, sizeof(expected), "%s/.suzaku-", root);
    if (strncmp(path, expected, strlen(expected)) != 0) { return real_fsync(fd); }
    char armed[PATH_MAX], entered[PATH_MAX], released[PATH_MAX], finished[PATH_MAX], failed[PATH_MAX];
    snprintf(armed, sizeof(armed), "%s/control-io.arm", root);
    snprintf(entered, sizeof(entered), "%s/control-io.entered", root);
    snprintf(released, sizeof(released), "%s/control-io.release", root);
    snprintf(finished, sizeof(finished), "%s/control-io.finished", root);
    snprintf(failed, sizeof(failed), "%s/control-io.fail", root);
    if (rename(armed, entered) != 0) { return real_fsync(fd); }
    struct timespec pause = {0, 10000000};
    /* Bound the fixture even when its observer fails; never an unbounded wait. */
    for (int attempt = 0; attempt < 300 && access(released, F_OK) != 0; attempt++) {
        nanosleep(&pause, NULL);
    }
    int result;
    if (access(failed, F_OK) == 0) { errno = EIO; result = -1; }
    else { result = real_fsync(fd); }
    int saved_errno = errno;
    int done = open(finished, O_WRONLY | O_CREAT | O_EXCL | O_CLOEXEC, 0600);
    if (done >= 0) { close(done); }
    errno = saved_errno;
    return result;
}
