#ifndef STARRY_RUNTIME_H
#define STARRY_RUNTIME_H

typedef unsigned long long SIZE_T;

void *rt_alloc(SIZE_T size);
void rt_free(void *ptr);
void rt_write_stdout(const char *data, SIZE_T n);
void rt_write_stderr(const char *data, SIZE_T n);
void rt_fatal_exit(unsigned int code);

#endif