#ifndef _SYS_SYSCTL_H
#define _SYS_SYSCTL_H

#include <sys/types.h>

#define CTL_HW 6
#define HW_NCPU 3

int sysctl(int *name, unsigned int namelen, void *oldp, size_t *oldlenp, void *newp, size_t newlen);
int sysctlbyname(const char *name, void *oldp, size_t *oldlenp, void *newp, size_t newlen);

#endif /* _SYS_SYSCTL_H */
