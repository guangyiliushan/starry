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

// %g 风格：`precision` 位有效数字。指数落在 [-4, precision) 用普通十进制
// 记法，否则用科学记数法；有效数字的尾随零先去掉，所以 100.0 → "100"、
// 0.5 → "0.5"、3.25 → "3.25"。输出确定性：同一输入恒同一串。
static char *fmt_double(double v, int precision) {
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

    // 有效数字，高位在前；rev 是低位在前，所以「尾随零」在 rev 头部
    char digits[32];
    int dn = 0;
    {
        char rev[32];
        int rn = 0;
        unsigned long long u = rounded;
        while (u) { rev[rn++] = (char)('0' + (int)(u % 10ULL)); u /= 10ULL; }
        // rounded ∈ [scale, 10·scale) 恒有 precision 位，无需补位
        int skip = 0;
        while (skip + 1 < rn && rev[skip] == '0') skip++;
        for (int k = 0; k < rn - skip; k++) digits[k] = rev[rn - 1 - k];
        dn = rn - skip;
    }

    if (e >= -4 && e < precision) {
        if (e < 0) {
            // 0.<-e-1 个 0><digits>
            buf[i++] = '0';
            buf[i++] = '.';
            for (int k = 0; k < -e - 1; k++) buf[i++] = '0';
            for (int k = 0; k < dn; k++) buf[i++] = digits[k];
        } else {
            int int_len = e + 1;
            if (int_len >= dn) {
                // 整数位不够长：补零到 e+1 位，无小数部
                for (int k = 0; k < dn; k++) buf[i++] = digits[k];
                for (int k = dn; k < int_len; k++) buf[i++] = '0';
            } else {
                for (int k = 0; k < int_len; k++) buf[i++] = digits[k];
                buf[i++] = '.';
                for (int k = int_len; k < dn; k++) buf[i++] = digits[k];
            }
        }
    } else {
        // 科学记数法：d[.rest]e<exp>
        buf[i++] = digits[0];
        if (dn > 1) {
            buf[i++] = '.';
            for (int k = 1; k < dn; k++) buf[i++] = digits[k];
        }
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
