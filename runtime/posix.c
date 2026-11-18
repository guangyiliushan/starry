#include <errno.h>
#include <stdlib.h>
#include <unistd.h>

typedef unsigned long long SIZE_T;

void rt_fatal_exit(unsigned int code);


void *rt_alloc(SIZE_T size) {
    void *p = malloc(size);
    if (!p) rt_fatal_exit(134u);
    return p;
}

void rt_free(void *p) {
    free(p);
}

static int write_all(int fd, const char *data, SIZE_T n) {
    SIZE_T done = 0;
    while (done < n) {
        ssize_t written = write(fd, data + done, (size_t)(n - done));
        if (written < 0) {
            if (errno == EINTR) continue;
            return 0;
        }
        if (written == 0) return 0;
        done += (SIZE_T)written;
    }
    return 1;
}

void rt_write_stdout(const char *data, SIZE_T n) {
    write_all(STDOUT_FILENO, data, n);
}

void rt_write_stderr(const char *data, SIZE_T n) {
    write_all(STDERR_FILENO, data, n);
}

void rt_fatal_exit(unsigned int code) {
    _exit((int)code);
}