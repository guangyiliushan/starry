
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
 * naked ret 导致 ACCESS_VIOLATION）。
 *
 * P0 回归修复（stack_probe_1_1mb/1_2mb 抓出）：探测方向必须**向下**——
 * 只有当前 guard 页被触碰时 OS 才自动增长栈（commit 该页并把 guard 下移
 * 一页）；旧实现从最深目标页**向上**探测，第一触就落在未提交的 reserve
 * 页上，直接 ACCESS_VIOLATION（最小 C 复现与本 runtime 无关地复现）。
 *
 * 契约：RAX 入/出 = 帧字节数（clang 随后 `sub rsp, rax`）；RCX/R10 为
 * volatile scratch。降序逐页触碰保证不跳过任何 guard 页。 */
__attribute__((naked)) void __chkstk(void) {
    __asm__ volatile(
        ".intel_syntax noprefix\n"
        "push rcx\n"
        "push rax\n"
        "cmp rax, 0x1000\n"
        "jle 2f\n"
        "mov rcx, rsp\n"
        "sub rcx, rax\n"
        "and rcx, -0x1000\n"     /* 最低待保证页（向下取整） */
        "mov r10, rsp\n"
        "sub r10, 0x1000\n"      /* 第一触：rsp 下方一页 = 当前 guard */
        "1:\n"
        "cmp r10, rcx\n"
        "jb 2f\n"
        "mov al, byte ptr [r10]\n"
        "sub r10, 0x1000\n"
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