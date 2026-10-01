#ifndef _MATH_H
#define _MATH_H

#define HUGE_VAL (__builtin_huge_val())
#define INFINITY (__builtin_inff())
#define NAN (__builtin_nanf(""))

#define isnan(x) ((x) != (x))
#define isinf(x) (fabs(x) == INFINITY)
#define isfinite(x) ((x) == (x) && fabs(x) != INFINITY)
#define signbit(x) ((x) < 0.0 || (1.0 / (x) < 0.0))

double sin(double x);
double cos(double x);
double tan(double x);
double asin(double x);
double acos(double x);
double atan(double x);
double atan2(double y, double x);

double sinh(double x);
double cosh(double x);
double tanh(double x);

double exp(double x);
double log(double x);
double log10(double x);
double log2(double x);
double pow(double x, double y);
double sqrt(double x);
double cbrt(double x);

double ceil(double x);
double floor(double x);
double round(double x);
double trunc(double x);
double fabs(double x);
double fmod(double x, double y);

float fabsf(float x);
float sqrtf(float x);
float sinf(float x);
float cosf(float x);

#endif /* _MATH_H */
