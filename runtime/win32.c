
typedef unsigned long long SIZE_T;
typedef unsigned long DWORD;
typedef void *HANDLE;

__declspec(dllimport) HANDLE GetProcessHeap(void);
__declspec(dllimport) void *HeapAlloc(HANDLE, DWORD, SIZE_T);
__declspec(dllimport) int HeapFree(HANDLE, DWORD, void *);
__declspec(dllimport) HANDLE GetStdHandle(DWORD);
__declspec(dllimport) int WriteFile(HANDLE, const void *, DWORD, DWORD *, void *);
__declspec(dllimport) void ExitProcess(unsigned int);

int main(void);
void rt_fatal_exit(unsigned int code);

void *rt_alloc(SIZE_T size) {
    void *p = HeapAlloc(GetProcessHeap(), 0, size);
    if (!p) rt_fatal_exit(134u);
    return p;
}

void rt_free(void *p) {
    HeapFree(GetProcessHeap(), 0, p);
}

static int write_all(HANDLE handle, const char *data, SIZE_T n) {
    SIZE_T done = 0;
    while (done < n) {
        DWORD chunk = (DWORD)(n - done);
        DWORD written = 0;
        if (!WriteFile(handle, data + done, chunk, &written, 0) || written == 0)
            return 0;
        done += written;
    }
    return 1;
}

void rt_write_stdout(const char *data, SIZE_T n) {
    write_all(GetStdHandle((DWORD)-11), data, n); // STD_OUTPUT_HANDLE
}

void rt_write_stderr(const char *data, SIZE_T n) {
    write_all(GetStdHandle((DWORD)-12), data, n); // STD_ERROR_HANDLE
}

void rt_fatal_exit(unsigned int code) {
    ExitProcess(code);
}

// -nostdlib needs these CRT-like symbols because -O2 may synthesize calls.
void *memset(void *dst, int val, unsigned long long n) {
    unsigned char *d = (unsigned char *)dst;
    for (unsigned long long i = 0; i < n; i++) d[i] = (unsigned char)val;
    return dst;
}

void *memcpy(void *dst, const void *src, unsigned long long n) {
    char *d = (char *)dst;
    const char *s = (const char *)src;
    for (unsigned long long i = 0; i < n; i++) d[i] = s[i];
    return dst;
}

unsigned long long strlen(const char *s) {
    unsigned long long n = 0;
    while (s[n]) n++;
    return n;
}

/* P1-A0-0：MSVC x64 __chkstk 真实逐页探测（N1 修复——1.2MB 栈帧
 * naked ret 导致 ACCESS_VIOLATION）。保留 RCX/RAX，>1MB 帧仍工作。 */
__attribute__((naked)) void __chkstk(void) {
    __asm__ volatile(
        ".intel_syntax noprefix\n"
        "push rcx\n"
        "push rax\n"
        "cmp rax, 0x1000\n"
        "jle 1f\n"
        "mov rcx, rsp\n"
        "sub rcx, rax\n"
        "and rcx, -0x1000\n"
        "1:\n"
        "cmp rcx, rsp\n"
        "jae 2f\n"
        "mov al, byte ptr [rcx]\n"
        "add rcx, 0x1000\n"
        "jmp 1b\n"
        "2:\n"
        "pop rax\n"
        "pop rcx\n"
        "ret\n"
    );
}

int _fltused;

void mainCRTStartup(void) {
    ExitProcess((unsigned int)main());
}