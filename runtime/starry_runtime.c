// Minimal Starry runtime. Self-contained on Windows: no C headers and no CRT;
// starryc build links this file with clang -nostdlib -lkernel32.
typedef unsigned long DWORD;
typedef unsigned long long SIZE_T;
typedef void *HANDLE;

__declspec(dllimport) HANDLE GetProcessHeap(void);
__declspec(dllimport) void *HeapAlloc(HANDLE, DWORD, SIZE_T);
__declspec(dllimport) HANDLE GetStdHandle(DWORD);
__declspec(dllimport) int WriteFile(HANDLE, const void *, DWORD, DWORD *, void *);
__declspec(dllimport) void ExitProcess(unsigned int);

int _fltused;
int main(void);

void mainCRTStartup(void) {
    int code = main();
    ExitProcess((unsigned int)code);
}

static void *xalloc(SIZE_T n) { return HeapAlloc(GetProcessHeap(), 0, n); }

static SIZE_T cstr_len(const char *s) {
    const char *p = s;
    while (*p) p++;
    return (SIZE_T)(p - s);
}

static void cstr_copy(char *dst, const char *src, SIZE_T n) {
    for (SIZE_T i = 0; i < n; i++) dst[i] = src[i];
}

static void write_str(const char *s) {
    DWORD written = 0;
    HANDLE out = GetStdHandle((DWORD)-11); // STD_OUTPUT_HANDLE
    DWORD n = (DWORD)cstr_len(s);
    WriteFile(out, s, n, &written, 0);
}

static void write_line(const char *s) {
    write_str(s);
    write_str("\n");
}

static char *from_buf(const char *buf, SIZE_T n) {
    char *out = (char *)xalloc(n + 1);
    if (!out) return 0;
    cstr_copy(out, buf, n);
    out[n] = 0;
    return out;
}

static char *fmt_i64(long long v) {
    char buf[32];
    unsigned long long u;
    int i = 32;
    int neg = 0;
    if (v < 0) {
        neg = 1;
        u = 0ULL - (unsigned long long)v;
    } else {
        u = (unsigned long long)v;
    }
    if (u == 0) return from_buf("0", 1);
    while (u) {
        buf[--i] = (char)('0' + (int)(u % 10ULL));
        u /= 10ULL;
    }
    if (neg) buf[--i] = '-';
    return from_buf(buf + i, (SIZE_T)(32 - i));
}

static char *fmt_double(double v, int precision) {
    // Small %g-style formatter: enough for deterministic sample output.
    char buf[96];
    int i = 0;
    if (v != v) return from_buf("nan", 3);
    if (v == 1.0 / 0.0) return from_buf("inf", 3);
    if (v == -1.0 / 0.0) return from_buf("-inf", 4);
    if (v == 0.0) return from_buf(v < 0.0 ? "-0" : "0", v < 0.0 ? 2 : 1);
    if (v < 0.0) {
        buf[i++] = '-';
        v = -v;
    }
    int e = 0;
    while (v >= 10.0) { v /= 10.0; e++; }
    while (v < 1.0) { v *= 10.0; e--; }
    unsigned long long scale = 1;
    for (int k = 1; k < precision; k++) scale *= 10ULL;
    unsigned long long rounded = (unsigned long long)(v * (double)scale + 0.5);
    if (rounded == scale * 10ULL) { rounded = scale; e++; }
    char digits[32];
    int dn = 0;
    while (rounded) { digits[dn++] = (char)('0' + (int)(rounded % 10ULL)); rounded /= 10ULL; }
    while (dn < precision) digits[dn++] = '0';
    for (int k = 0; k < dn; k++) buf[i + k] = digits[dn - 1 - k];
    i += dn;
    int last = i - 1;
    while (last > 0 && buf[last] == '0') last--;
    if (last > 0 && buf[last] == '.' ) last--;
    i = last + 1;
    if (e != 0) {
        buf[i++] = 'e';
        if (e < 0) { buf[i++] = '-'; e = -e; }
        char eb[12];
        int en = 0;
        while (e) { eb[en++] = (char)('0' + e % 10); e /= 10; }
        while (en) buf[i++] = eb[--en];
    }
    return from_buf(buf, (SIZE_T)i);
}

static char *dup_string(const char *s) {
    SIZE_T n = cstr_len(s);
    char *out = (char *)xalloc(n + 1);
    if (!out) return 0;
    cstr_copy(out, s, n);
    out[n] = 0;
    return out;
}

char *starry_str_conv_i64(long long v) { return fmt_i64(v); }
char *starry_str_conv_i32(int v) { return fmt_i64(v); }
char *starry_str_conv_i8(signed char v) { return fmt_i64(v); }
char *starry_str_conv_i1(int v) { return dup_string(v ? "true" : "false"); }
char *starry_str_conv_f64(double v) { return fmt_double(v, 15); }
char *starry_str_conv_f32(float v) { return fmt_double((double)v, 7); }
char *starry_str_conv_string(const char *s) { return dup_string(s); }

char *starry_str_concat(const char *a, const char *b) {
    SIZE_T na = cstr_len(a), nb = cstr_len(b);
    char *out = (char *)xalloc(na + nb + 1);
    if (!out) return 0;
    cstr_copy(out, a, na);
    cstr_copy(out + na, b, nb);
    out[na + nb] = 0;
    return out;
}

void starry_print_i64(long long v) { write_line(fmt_i64(v)); }
void starry_print_i32(int v) { write_line(fmt_i64(v)); }
void starry_print_i8(signed char v) { write_line(fmt_i64(v)); }
void starry_print_i1(int v) { write_line(v ? "true" : "false"); }
void starry_print_f64(double v) { write_line(fmt_double(v, 15)); }
void starry_print_f32(float v) { write_line(fmt_double((double)v, 7)); }
void starry_print_string(const char *s) { write_line(s); }
