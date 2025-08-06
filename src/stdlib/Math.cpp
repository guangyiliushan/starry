#include "starry/stdlib/Math.h"
#include <cmath>
#include <random>
#include <algorithm>
#include <numeric>
#include <limits>

namespace starry {
namespace stdlib {

// 数学常量
const double Math::PI = 3.14159265358979323846;
const double Math::E = 2.71828182845904523536;
const double Math::SQRT2 = 1.41421356237309504880;
const double Math::SQRT3 = 1.73205080756887729353;
const double Math::LN2 = 0.69314718055994530942;
const double Math::LN10 = 2.30258509299404568402;

// 随机数生成器
static std::random_device rd;
static std::mt19937 gen(rd());

// 基础数学函数
double Math::abs(double x) {
    return std::abs(x);
}

int Math::abs(int x) {
    return std::abs(x);
}

double Math::sqrt(double x) {
    if (x < 0) {
        throw std::invalid_argument("sqrt: 负数没有实数平方根");
    }
    return std::sqrt(x);
}

double Math::pow(double base, double exponent) {
    return std::pow(base, exponent);
}

double Math::exp(double x) {
    return std::exp(x);
}

double Math::log(double x) {
    if (x <= 0) {
        throw std::invalid_argument("log: 参数必须为正数");
    }
    return std::log(x);
}

double Math::log10(double x) {
    if (x <= 0) {
        throw std::invalid_argument("log10: 参数必须为正数");
    }
    return std::log10(x);
}

double Math::log2(double x) {
    if (x <= 0) {
        throw std::invalid_argument("log2: 参数必须为正数");
    }
    return std::log2(x);
}

// 三角函数
double Math::sin(double x) {
    return std::sin(x);
}

double Math::cos(double x) {
    return std::cos(x);
}

double Math::tan(double x) {
    return std::tan(x);
}

double Math::asin(double x) {
    if (x < -1.0 || x > 1.0) {
        throw std::invalid_argument("asin: 参数必须在[-1, 1]范围内");
    }
    return std::asin(x);
}

double Math::acos(double x) {
    if (x < -1.0 || x > 1.0) {
        throw std::invalid_argument("acos: 参数必须在[-1, 1]范围内");
    }
    return std::acos(x);
}

double Math::atan(double x) {
    return std::atan(x);
}

double Math::atan2(double y, double x) {
    return std::atan2(y, x);
}

// 双曲函数
double Math::sinh(double x) {
    return std::sinh(x);
}

double Math::cosh(double x) {
    return std::cosh(x);
}

double Math::tanh(double x) {
    return std::tanh(x);
}

double Math::asinh(double x) {
    return std::asinh(x);
}

double Math::acosh(double x) {
    if (x < 1.0) {
        throw std::invalid_argument("acosh: 参数必须大于等于1");
    }
    return std::acosh(x);
}

double Math::atanh(double x) {
    if (x <= -1.0 || x >= 1.0) {
        throw std::invalid_argument("atanh: 参数必须在(-1, 1)范围内");
    }
    return std::atanh(x);
}

// 取整函数
double Math::floor(double x) {
    return std::floor(x);
}

double Math::ceil(double x) {
    return std::ceil(x);
}

double Math::round(double x) {
    return std::round(x);
}

double Math::trunc(double x) {
    return std::trunc(x);
}

// 比较函数
double Math::max(double a, double b) {
    return std::max(a, b);
}

double Math::min(double a, double b) {
    return std::min(a, b);
}

int Math::max(int a, int b) {
    return std::max(a, b);
}

int Math::min(int a, int b) {
    return std::min(a, b);
}

// 符号函数
int Math::sign(double x) {
    if (x > 0) return 1;
    if (x < 0) return -1;
    return 0;
}

// 判断函数
bool Math::isNaN(double x) {
    return std::isnan(x);
}

bool Math::isInfinite(double x) {
    return std::isinf(x);
}

bool Math::isFinite(double x) {
    return std::isfinite(x);
}

// 随机数函数
double Math::random() {
    static std::uniform_real_distribution<double> dis(0.0, 1.0);
    return dis(gen);
}

int Math::randomInt(int min, int max) {
    if (min > max) {
        throw std::invalid_argument("randomInt: min不能大于max");
    }
    std::uniform_int_distribution<int> dis(min, max);
    return dis(gen);
}

double Math::randomDouble(double min, double max) {
    if (min > max) {
        throw std::invalid_argument("randomDouble: min不能大于max");
    }
    std::uniform_real_distribution<double> dis(min, max);
    return dis(gen);
}

void Math::setSeed(unsigned int seed) {
    gen.seed(seed);
}

// 角度转换
double Math::toRadians(double degrees) {
    return degrees * PI / 180.0;
}

double Math::toDegrees(double radians) {
    return radians * 180.0 / PI;
}

// 数值处理
double Math::clamp(double value, double min, double max) {
    if (min > max) {
        throw std::invalid_argument("clamp: min不能大于max");
    }
    return std::max(min, std::min(value, max));
}

double Math::lerp(double a, double b, double t) {
    return a + t * (b - a);
}

double Math::map(double value, double fromMin, double fromMax, double toMin, double toMax) {
    if (fromMin == fromMax) {
        throw std::invalid_argument("map: fromMin不能等于fromMax");
    }
    double t = (value - fromMin) / (fromMax - fromMin);
    return lerp(toMin, toMax, t);
}

// 统计函数
double Math::sum(const std::vector<double>& values) {
    return std::accumulate(values.begin(), values.end(), 0.0);
}

double Math::mean(const std::vector<double>& values) {
    if (values.empty()) {
        throw std::invalid_argument("mean: 空向量没有平均值");
    }
    return sum(values) / values.size();
}

double Math::median(std::vector<double> values) {
    if (values.empty()) {
        throw std::invalid_argument("median: 空向量没有中位数");
    }
    
    std::sort(values.begin(), values.end());
    size_t n = values.size();
    
    if (n % 2 == 0) {
        return (values[n/2 - 1] + values[n/2]) / 2.0;
    } else {
        return values[n/2];
    }
}

double Math::variance(const std::vector<double>& values) {
    if (values.size() < 2) {
        throw std::invalid_argument("variance: 至少需要2个值");
    }
    
    double m = mean(values);
    double sum_sq_diff = 0.0;
    
    for (double value : values) {
        double diff = value - m;
        sum_sq_diff += diff * diff;
    }
    
    return sum_sq_diff / (values.size() - 1);
}

double Math::standardDeviation(const std::vector<double>& values) {
    return sqrt(variance(values));
}

// 数论函数
int Math::gcd(int a, int b) {
    a = abs(a);
    b = abs(b);
    
    while (b != 0) {
        int temp = b;
        b = a % b;
        a = temp;
    }
    
    return a;
}

int Math::lcm(int a, int b) {
    if (a == 0 || b == 0) {
        return 0;
    }
    return abs(a * b) / gcd(a, b);
}

bool Math::isPrime(int n) {
    if (n < 2) return false;
    if (n == 2) return true;
    if (n % 2 == 0) return false;
    
    for (int i = 3; i * i <= n; i += 2) {
        if (n % i == 0) return false;
    }
    
    return true;
}

long long Math::factorial(int n) {
    if (n < 0) {
        throw std::invalid_argument("factorial: 参数不能为负数");
    }
    if (n > 20) {
        throw std::invalid_argument("factorial: 参数过大，会导致溢出");
    }
    
    long long result = 1;
    for (int i = 2; i <= n; ++i) {
        result *= i;
    }
    
    return result;
}

long long Math::fibonacci(int n) {
    if (n < 0) {
        throw std::invalid_argument("fibonacci: 参数不能为负数");
    }
    if (n == 0) return 0;
    if (n == 1) return 1;
    
    long long a = 0, b = 1;
    for (int i = 2; i <= n; ++i) {
        long long temp = a + b;
        a = b;
        b = temp;
    }
    
    return b;
}

// 组合数学
long long Math::combination(int n, int r) {
    if (r < 0 || r > n) {
        return 0;
    }
    if (r == 0 || r == n) {
        return 1;
    }
    
    // 优化：C(n,r) = C(n,n-r)
    if (r > n - r) {
        r = n - r;
    }
    
    long long result = 1;
    for (int i = 0; i < r; ++i) {
        result = result * (n - i) / (i + 1);
    }
    
    return result;
}

long long Math::permutation(int n, int r) {
    if (r < 0 || r > n) {
        return 0;
    }
    
    long long result = 1;
    for (int i = 0; i < r; ++i) {
        result *= (n - i);
    }
    
    return result;
}

} // namespace stdlib
} // namespace starry